use serde_json::Value;
use tracon_core::event::{AgentEvent, EventKind, EventSource};
use tracon_core::now_iso;

use crate::{sanitize_session_id, truncate};

pub const AGENT_NAME: &str = "gemini";

/// Normalize one Gemini CLI hook payload. Gemini's payload shape overlaps
/// with Claude Code's (both carry session_id and hook_event_name), so these
/// arrive on a dedicated /ingest/gemini route instead of shape detection.
/// BeforeModel/AfterModel fire per model request and are deliberately
/// dropped: too noisy for an audit timeline.
pub fn events_from_hook_payload(payload: &Value) -> Vec<AgentEvent> {
    let Some(hook_event) = payload.get("hook_event_name").and_then(Value::as_str) else {
        return Vec::new();
    };
    let session_id = sanitize_session_id(
        payload
            .get("session_id")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    );
    let ts = payload
        .get("timestamp")
        .and_then(Value::as_str)
        .map(String::from)
        .unwrap_or_else(now_iso);
    let cwd = payload.get("cwd").and_then(Value::as_str).map(String::from);

    let kind = match hook_event {
        "BeforeTool" => EventKind::ToolCall,
        "AfterTool" => EventKind::ToolResult,
        "SessionStart" => EventKind::SessionStart,
        "SessionEnd" => EventKind::SessionEnd,
        "BeforeAgent" | "AfterAgent" | "Notification" | "PreCompress" => EventKind::Other,
        // BeforeModel / AfterModel / BeforeToolSelection: skipped on purpose.
        _ => return Vec::new(),
    };

    let raw_tool = tool_name(payload);
    let command = tool_command(payload);
    let summary = summary_for(hook_event, raw_tool.as_deref(), command.as_deref(), payload);
    let display_tool = raw_tool.map(|t| normalize_tool(&t));
    let flag = command.as_deref().and_then(crate::danger::assess_command);
    let dedupe_key = Some(format!(
        "{session_id}|{hook_event}|{ts}|{}",
        prefix(&summary)
    ));

    let mut events = vec![AgentEvent {
        id: None,
        agent: AGENT_NAME.into(),
        session_id: session_id.clone(),
        ts: ts.clone(),
        kind,
        source: EventSource::Hook,
        cwd: cwd.clone(),
        tool_name: display_tool,
        summary: Some(summary),
        flag,
        payload: payload.clone(),
        dedupe_key,
    }];

    let install = command
        .as_deref()
        .and_then(crate::packages::detect_package_install);
    if let (Some(install), EventKind::ToolCall) = (install, kind) {
        let key = Some(format!("{session_id}|pkg|{ts}|{}", prefix(&install)));
        events.push(AgentEvent {
            id: None,
            agent: AGENT_NAME.into(),
            session_id,
            ts,
            kind: EventKind::PackageInstall,
            source: EventSource::Hook,
            cwd,
            tool_name: install.split_whitespace().next().map(String::from),
            summary: Some(install),
            flag: None,
            payload: payload.clone(),
            dedupe_key: key,
        });
    }
    events
}

fn tool_name(payload: &Value) -> Option<String> {
    payload
        .get("tool_name")
        .or_else(|| payload.get("toolName"))
        .or_else(|| payload.get("tool").and_then(|t| t.get("name")))
        .and_then(Value::as_str)
        .map(String::from)
}

fn tool_input(payload: &Value) -> Option<&Value> {
    payload
        .get("tool_input")
        .or_else(|| payload.get("toolArgs"))
        .or_else(|| payload.get("args"))
}

fn tool_command(payload: &Value) -> Option<String> {
    if tool_name(payload)?.as_str() != "run_shell_command" {
        return None;
    }
    tool_input(payload)?
        .get("command")
        .and_then(Value::as_str)
        .map(String::from)
}

fn normalize_tool(raw: &str) -> String {
    match raw {
        "run_shell_command" => "shell".into(),
        "write_file" => "Write".into(),
        "read_file" | "read_many_files" => "Read".into(),
        "replace" | "edit" => "Edit".into(),
        other => other.into(),
    }
}

fn summary_for(
    hook_event: &str,
    raw_tool: Option<&str>,
    command: Option<&str>,
    payload: &Value,
) -> String {
    if let Some(cmd) = command {
        return truncate(cmd, 300);
    }
    let file_path = tool_input(payload)
        .and_then(|i| i.get("file_path").or_else(|| i.get("path")))
        .and_then(Value::as_str);
    if let Some(path) = file_path {
        return truncate(path, 300);
    }
    raw_tool.unwrap_or(hook_event).to_string()
}

fn prefix(s: &str) -> String {
    s.chars().take(80).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn shell_tool_yields_call_and_package_events() {
        let payload = json!({
            "session_id": "g-sess",
            "hook_event_name": "BeforeTool",
            "timestamp": "2026-08-29T14:00:00Z",
            "cwd": "/work/ml",
            "tool_name": "run_shell_command",
            "tool_input": {"command": "uv pip install httpx"}
        });

        let events = events_from_hook_payload(&payload);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].agent, "gemini");
        assert_eq!(events[0].kind, EventKind::ToolCall);
        assert_eq!(events[0].tool_name.as_deref(), Some("shell"));
        assert_eq!(events[0].summary.as_deref(), Some("uv pip install httpx"));
        assert_eq!(events[1].kind, EventKind::PackageInstall);
    }

    #[test]
    fn file_tools_map_to_shared_names() {
        let payload = json!({
            "session_id": "g",
            "hook_event_name": "BeforeTool",
            "tool_name": "write_file",
            "tool_input": {"file_path": "src/train.py"}
        });
        let events = events_from_hook_payload(&payload);
        assert_eq!(events[0].tool_name.as_deref(), Some("Write"));
        assert_eq!(events[0].summary.as_deref(), Some("src/train.py"));
    }

    #[test]
    fn model_events_are_dropped_and_danger_flagged() {
        let model = json!({"session_id": "g", "hook_event_name": "BeforeModel"});
        assert!(events_from_hook_payload(&model).is_empty());

        let danger = json!({
            "session_id": "g",
            "hook_event_name": "BeforeTool",
            "tool_name": "run_shell_command",
            "tool_input": {"command": "rm -rf ~/"}
        });
        assert!(events_from_hook_payload(&danger)[0].flag.is_some());
    }

    #[test]
    fn camel_case_fallbacks_are_read() {
        let payload = json!({
            "session_id": "g",
            "hook_event_name": "BeforeTool",
            "toolName": "run_shell_command",
            "toolArgs": {"command": "pip install requests"}
        });
        let events = events_from_hook_payload(&payload);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].tool_name.as_deref(), Some("shell"));
        assert_eq!(events[0].summary.as_deref(), Some("pip install requests"));
        assert_eq!(events[1].kind, EventKind::PackageInstall);

        let nested = json!({
            "session_id": "g",
            "hook_event_name": "AfterTool",
            "tool": {"name": "read_file"},
            "args": {"path": "README.md"}
        });
        let events = events_from_hook_payload(&nested);
        assert_eq!(events[0].kind, EventKind::ToolResult);
        assert_eq!(events[0].tool_name.as_deref(), Some("Read"));
        assert_eq!(events[0].summary.as_deref(), Some("README.md"));
    }

    #[test]
    fn session_id_is_sanitized_and_file_paths_truncated() {
        let long_path = format!("/{}", "a".repeat(400));
        let payload = json!({
            "session_id": "../../etc",
            "hook_event_name": "BeforeTool",
            "tool_name": "write_file",
            "tool_input": {"file_path": long_path}
        });
        let events = events_from_hook_payload(&payload);
        assert_eq!(events[0].session_id, ".._.._etc");
        assert_eq!(events[0].summary.as_deref().map(str::len), Some(303));

        let missing = json!({"hook_event_name": "SessionStart"});
        assert_eq!(events_from_hook_payload(&missing)[0].session_id, "unknown");
    }
}
