use serde_json::Value;
use tracon_core::event::{AgentEvent, EventKind, EventSource};
use tracon_core::now_iso;

pub const AGENT_NAME: &str = "claude-code";

/// All events one hook payload yields: the normalized event itself, plus a
/// derived package_install event when a Bash tool call runs a package manager.
pub fn events_from_hook_payload(payload: &Value) -> Vec<AgentEvent> {
    let Some(event) = normalize_hook_payload(payload) else {
        return Vec::new();
    };
    let package_event = derive_package_event(&event, payload);
    let mut events = vec![event];
    events.extend(package_event);
    events
}

/// Only the moment the command starts (PreToolUse) produces a package event,
/// so the PostToolUse for the same call doesn't double-report the install.
fn derive_package_event(event: &AgentEvent, payload: &Value) -> Option<AgentEvent> {
    let is_bash_call =
        event.kind == EventKind::ToolCall && event.tool_name.as_deref() == Some("Bash");
    if !is_bash_call {
        return None;
    }
    let command = payload.get("tool_input")?.get("command")?.as_str()?;
    let install = crate::packages::detect_package_install(command)?;
    let dedupe_key =
        str_field(payload, "tool_use_id").map(|id| format!("{}|pkg|{id}", event.session_id));

    Some(AgentEvent {
        id: None,
        agent: AGENT_NAME.into(),
        session_id: event.session_id.clone(),
        ts: event.ts.clone(),
        kind: EventKind::PackageInstall,
        source: event.source,
        cwd: event.cwd.clone(),
        tool_name: install.split_whitespace().next().map(String::from),
        summary: Some(install),
        flag: None,
        payload: payload.clone(),
        dedupe_key,
    })
}

/// Normalize one Claude Code hook payload (HTTP hook body or spool line) into an event.
/// Returns None for payloads that don't look like hook events; the caller drops them
/// silently because the recorder must never give the agent a reason to fail.
pub fn normalize_hook_payload(payload: &Value) -> Option<AgentEvent> {
    let hook_event = payload.get("hook_event_name")?.as_str()?.to_string();
    let session_id = str_field(payload, "session_id").unwrap_or_else(|| "unknown".into());
    let tool_name = str_field(payload, "tool_name");
    let flag = bash_command(payload, tool_name.as_deref())
        .and_then(|cmd| crate::danger::assess_command(&cmd));

    Some(AgentEvent {
        id: None,
        agent: AGENT_NAME.into(),
        session_id: session_id.clone(),
        ts: now_iso(),
        kind: kind_for(&hook_event),
        source: EventSource::Hook,
        cwd: str_field(payload, "cwd"),
        summary: summary_for(&hook_event, tool_name.as_deref(), payload),
        tool_name,
        flag,
        payload: payload.clone(),
        dedupe_key: dedupe_key_for(&hook_event, &session_id, payload),
    })
}

fn kind_for(hook_event: &str) -> EventKind {
    match hook_event {
        "SessionStart" => EventKind::SessionStart,
        "SessionEnd" => EventKind::SessionEnd,
        "UserPromptSubmit" => EventKind::Prompt,
        "PreToolUse" => EventKind::ToolCall,
        "PostToolUse" | "PostToolUseFailure" => EventKind::ToolResult,
        "PermissionRequest" | "PermissionDenied" => EventKind::Approval,
        "ConfigChange" => EventKind::ConfigChange,
        _ => EventKind::Other,
    }
}

/// One line for the timeline: the command for Bash, the path for file tools,
/// a prompt excerpt for prompts, the event name otherwise.
fn summary_for(hook_event: &str, tool_name: Option<&str>, payload: &Value) -> Option<String> {
    let tool_input = payload.get("tool_input");

    let from_tool = match tool_name {
        Some("Bash") => input_str(tool_input, "command"),
        Some("Edit") | Some("Write") | Some("MultiEdit") | Some("NotebookEdit") | Some("Read") => {
            input_str(tool_input, "file_path")
        }
        Some(other) => Some(other.to_string()),
        None => None,
    };
    if let Some(text) = from_tool {
        return Some(truncate(&text, 300));
    }

    if hook_event == "UserPromptSubmit" {
        if let Some(prompt) = str_field(payload, "prompt") {
            return Some(truncate(&prompt, 200));
        }
    }
    Some(hook_event.to_string())
}

/// PreToolUse and PostToolUse for the same call share a tool_use_id but differ in
/// event name, so the pair survives while an HTTP-plus-spool duplicate collapses.
fn dedupe_key_for(hook_event: &str, session_id: &str, payload: &Value) -> Option<String> {
    if let Some(id) = str_field(payload, "tool_use_id") {
        return Some(format!("{session_id}|{hook_event}|{id}"));
    }
    if let Some(id) = str_field(payload, "prompt_id") {
        return Some(format!("{session_id}|{hook_event}|{id}"));
    }
    None
}

fn bash_command(payload: &Value, tool_name: Option<&str>) -> Option<String> {
    if tool_name != Some("Bash") {
        return None;
    }
    input_str(payload.get("tool_input"), "command")
}

fn str_field(value: &Value, key: &str) -> Option<String> {
    value.get(key)?.as_str().map(String::from)
}

fn input_str(tool_input: Option<&Value>, key: &str) -> Option<String> {
    tool_input?.get(key)?.as_str().map(String::from)
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
    fn normalizes_bash_pre_tool_use() {
        let payload = json!({
            "session_id": "sess-1",
            "hook_event_name": "PreToolUse",
            "cwd": "/home/dev/project",
            "tool_name": "Bash",
            "tool_use_id": "toolu_123",
            "tool_input": {"command": "npm install express"}
        });

        let event = normalize_hook_payload(&payload).unwrap();
        assert_eq!(event.agent, AGENT_NAME);
        assert_eq!(event.kind, EventKind::ToolCall);
        assert_eq!(event.tool_name.as_deref(), Some("Bash"));
        assert_eq!(event.summary.as_deref(), Some("npm install express"));
        assert_eq!(
            event.dedupe_key.as_deref(),
            Some("sess-1|PreToolUse|toolu_123")
        );
    }

    #[test]
    fn pre_and_post_of_same_call_keep_distinct_keys() {
        let pre = json!({"session_id": "s", "hook_event_name": "PreToolUse", "tool_use_id": "t1"});
        let post =
            json!({"session_id": "s", "hook_event_name": "PostToolUse", "tool_use_id": "t1"});
        let pre_key = normalize_hook_payload(&pre).unwrap().dedupe_key;
        let post_key = normalize_hook_payload(&post).unwrap().dedupe_key;
        assert_ne!(pre_key, post_key);
    }

    #[test]
    fn ignores_non_hook_payloads() {
        assert!(normalize_hook_payload(&json!({"foo": "bar"})).is_none());
    }
}
