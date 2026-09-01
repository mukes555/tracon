use serde_json::Value;
use tracon_core::event::{AgentEvent, EventKind, EventSource};
use tracon_core::now_iso;

use crate::claude::AGENT_NAME;

/// Parse one line of a Claude Code transcript (~/.claude/projects/**/*.jsonl)
/// into zero or more events.
///
/// The format is undocumented and drifts, so everything here is best-effort:
/// unknown record types and missing fields yield an empty Vec, never an error.
/// Dedupe keys are built to MATCH the hook adapter's keys, so whichever source
/// records an action first wins and the other collapses into it.
pub fn parse_transcript_line(line: &str) -> Vec<AgentEvent> {
    let Ok(row) = serde_json::from_str::<Value>(line) else {
        return Vec::new();
    };
    let Some(row_type) = row.get("type").and_then(Value::as_str) else {
        return Vec::new();
    };
    let Some(session_id) = row.get("sessionId").and_then(Value::as_str) else {
        return Vec::new();
    };

    let ctx = RowContext {
        session_id,
        ts: row
            .get("timestamp")
            .and_then(Value::as_str)
            .map(String::from)
            .unwrap_or_else(now_iso),
        cwd: row.get("cwd").and_then(Value::as_str).map(String::from),
    };

    match row_type {
        "assistant" => assistant_events(&row, &ctx),
        "user" => user_events(&row, &ctx),
        _ => Vec::new(),
    }
}

struct RowContext<'a> {
    session_id: &'a str,
    ts: String,
    cwd: Option<String>,
}

/// Assistant rows carry tool_use blocks: each one is a tool call the agent made.
fn assistant_events(row: &Value, ctx: &RowContext<'_>) -> Vec<AgentEvent> {
    let mut events = Vec::new();
    for block in content_blocks(row) {
        if block.get("type").and_then(Value::as_str) != Some("tool_use") {
            continue;
        }
        let tool_name = block.get("name").and_then(Value::as_str).unwrap_or("tool");
        let block_id = block.get("id").and_then(Value::as_str);
        let input = block.get("input");
        let summary = tool_summary(tool_name, input);

        let mut call = base_event(
            ctx,
            EventKind::ToolCall,
            Some(tool_name.to_string()),
            summary,
            block.clone(),
            block_id.map(|id| format!("{}|PreToolUse|{id}", ctx.session_id)),
        );
        if tool_name == "Bash" {
            call.flag = input
                .and_then(|i| i.get("command"))
                .and_then(Value::as_str)
                .and_then(crate::danger::assess_command);
        }
        events.push(call);

        events.extend(package_event(ctx, tool_name, input, &block, block_id));
    }
    events
}

/// User rows are either tool results coming back or an actual human prompt.
fn user_events(row: &Value, ctx: &RowContext<'_>) -> Vec<AgentEvent> {
    let results: Vec<AgentEvent> = content_blocks(row)
        .iter()
        .filter(|b| b.get("type").and_then(Value::as_str) == Some("tool_result"))
        .filter_map(|block| {
            let id = block.get("tool_use_id").and_then(Value::as_str)?;
            Some(base_event(
                ctx,
                EventKind::ToolResult,
                None,
                None,
                block.clone(),
                Some(format!("{}|PostToolUse|{id}", ctx.session_id)),
            ))
        })
        .collect();
    if !results.is_empty() {
        return results;
    }

    let Some(prompt) = prompt_text(row) else {
        return Vec::new();
    };
    let prompt_id = row
        .get("promptId")
        .or_else(|| row.get("uuid"))
        .and_then(Value::as_str);
    vec![base_event(
        ctx,
        EventKind::Prompt,
        None,
        Some(truncate(&prompt, 200)),
        row.clone(),
        prompt_id.map(|id| format!("{}|UserPromptSubmit|{id}", ctx.session_id)),
    )]
}

fn package_event(
    ctx: &RowContext<'_>,
    tool_name: &str,
    input: Option<&Value>,
    block: &Value,
    block_id: Option<&str>,
) -> Option<AgentEvent> {
    if tool_name != "Bash" {
        return None;
    }
    let command = input?.get("command")?.as_str()?;
    let install = crate::packages::detect_package_install(command)?;
    Some(base_event(
        ctx,
        EventKind::PackageInstall,
        install.split_whitespace().next().map(String::from),
        Some(install),
        block.clone(),
        block_id.map(|id| format!("{}|pkg|{id}", ctx.session_id)),
    ))
}

fn base_event(
    ctx: &RowContext<'_>,
    kind: EventKind,
    tool_name: Option<String>,
    summary: Option<String>,
    payload: Value,
    dedupe_key: Option<String>,
) -> AgentEvent {
    AgentEvent {
        id: None,
        agent: AGENT_NAME.into(),
        session_id: ctx.session_id.to_string(),
        ts: ctx.ts.clone(),
        kind,
        source: EventSource::LogTail,
        cwd: ctx.cwd.clone(),
        tool_name,
        summary,
        flag: None,
        payload,
        dedupe_key,
    }
}

fn content_blocks(row: &Value) -> Vec<Value> {
    row.get("message")
        .and_then(|m| m.get("content"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

/// A prompt is a user row whose content is plain text (string form or text blocks).
fn prompt_text(row: &Value) -> Option<String> {
    let content = row.get("message")?.get("content")?;
    if let Some(text) = content.as_str() {
        return non_empty(text);
    }
    let joined = content
        .as_array()?
        .iter()
        .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|b| b.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join(" ");
    non_empty(&joined)
}

fn non_empty(s: &str) -> Option<String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn tool_summary(tool_name: &str, input: Option<&Value>) -> Option<String> {
    let text = match tool_name {
        "Bash" => input?.get("command")?.as_str()?,
        "Edit" | "Write" | "MultiEdit" | "NotebookEdit" | "Read" => {
            input?.get("file_path")?.as_str()?
        }
        other => other,
    };
    Some(truncate(text, 300))
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let cut: String = s.chars().take(max_chars).collect();
    format!("{cut}...")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn assistant_bash_tool_use_yields_call_and_package_events() {
        let line = json!({
            "type": "assistant",
            "sessionId": "s1",
            "timestamp": "2026-08-29T10:00:00Z",
            "cwd": "/work/api",
            "message": {"content": [
                {"type": "text", "text": "Installing now."},
                {"type": "tool_use", "id": "toolu_9", "name": "Bash",
                 "input": {"command": "pnpm add zod"}}
            ]}
        })
        .to_string();

        let events = parse_transcript_line(&line);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].kind, EventKind::ToolCall);
        assert_eq!(
            events[0].dedupe_key.as_deref(),
            Some("s1|PreToolUse|toolu_9")
        );
        assert_eq!(events[1].kind, EventKind::PackageInstall);
        assert_eq!(events[1].summary.as_deref(), Some("pnpm add zod"));
        assert_eq!(events[0].ts, "2026-08-29T10:00:00Z");
    }

    #[test]
    fn transcript_tool_call_dedupes_against_hook_event() {
        let hook = json!({
            "session_id": "s1",
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_use_id": "toolu_9",
            "tool_input": {"command": "pnpm add zod"}
        });
        let hook_key = crate::claude::normalize_hook_payload(&hook)
            .unwrap()
            .dedupe_key;

        let line = json!({
            "type": "assistant",
            "sessionId": "s1",
            "message": {"content": [
                {"type": "tool_use", "id": "toolu_9", "name": "Bash",
                 "input": {"command": "pnpm add zod"}}
            ]}
        })
        .to_string();
        let transcript_key = parse_transcript_line(&line)[0].dedupe_key.clone();

        assert_eq!(hook_key, transcript_key);
    }

    #[test]
    fn prompt_keys_match_between_hook_and_transcript_sources() {
        let hook = json!({
            "session_id": "s1",
            "hook_event_name": "UserPromptSubmit",
            "prompt_id": "p-77",
            "prompt": "add tests"
        });
        let hook_key = crate::claude::normalize_hook_payload(&hook)
            .unwrap()
            .dedupe_key;

        let transcript = json!({
            "type": "user",
            "sessionId": "s1",
            "promptId": "p-77",
            "message": {"content": "add tests"}
        })
        .to_string();
        let transcript_key = parse_transcript_line(&transcript)[0].dedupe_key.clone();

        assert_eq!(hook_key, transcript_key);
    }

    #[test]
    fn package_keys_match_between_hook_and_transcript_sources() {
        let hook = json!({
            "session_id": "s1",
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_use_id": "t9",
            "tool_input": {"command": "npm install left-pad"}
        });
        let hook_events = crate::claude::events_from_hook_payload(&hook);
        assert_eq!(hook_events.len(), 2);
        let hook_pkg_key = hook_events[1].dedupe_key.clone();

        let transcript = json!({
            "type": "assistant",
            "sessionId": "s1",
            "message": {"content": [
                {"type": "tool_use", "id": "t9", "name": "Bash",
                 "input": {"command": "npm install left-pad"}}
            ]}
        })
        .to_string();
        let transcript_events = parse_transcript_line(&transcript);
        assert_eq!(transcript_events.len(), 2);

        assert_eq!(hook_pkg_key, transcript_events[1].dedupe_key);
    }

    #[test]
    fn user_prompt_row_becomes_prompt_event() {
        let line = json!({
            "type": "user",
            "sessionId": "s1",
            "uuid": "u-1",
            "message": {"content": "please add tests"}
        })
        .to_string();

        let events = parse_transcript_line(&line);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, EventKind::Prompt);
        assert_eq!(events[0].summary.as_deref(), Some("please add tests"));
    }

    #[test]
    fn user_tool_result_row_becomes_tool_result_event() {
        let line = json!({
            "type": "user",
            "sessionId": "s1",
            "message": {"content": [
                {"type": "tool_result", "tool_use_id": "toolu_9", "content": "done"}
            ]}
        })
        .to_string();

        let events = parse_transcript_line(&line);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, EventKind::ToolResult);
        assert_eq!(
            events[0].dedupe_key.as_deref(),
            Some("s1|PostToolUse|toolu_9")
        );
    }

    #[test]
    fn garbage_and_unknown_rows_are_ignored() {
        assert!(parse_transcript_line("not json at all").is_empty());
        assert!(parse_transcript_line("{\"type\":\"file-history-snapshot\"}").is_empty());
        assert!(parse_transcript_line("{}").is_empty());
    }
}
