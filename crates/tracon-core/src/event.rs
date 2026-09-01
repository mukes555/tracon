use serde::{Deserialize, Serialize};

/// One recorded thing an agent did, normalized across all agents and capture paths.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEvent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    /// Which agent produced it, e.g. "claude-code".
    pub agent: String,
    pub session_id: String,
    /// RFC 3339 UTC timestamp.
    pub ts: String,
    pub kind: EventKind,
    pub source: EventSource,
    pub cwd: Option<String>,
    pub tool_name: Option<String>,
    /// Human-readable one-liner for the timeline (the bash command, the file path, ...).
    pub summary: Option<String>,
    /// Danger-heuristic label ("destructive delete", ...) when the action deserves review.
    #[serde(default)]
    pub flag: Option<String>,
    /// The full original payload, kept verbatim for drill-down and future re-normalization.
    pub payload: serde_json::Value,
    /// Uniqueness key so the same event arriving twice (HTTP hook + spool) stores once.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dedupe_key: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    SessionStart,
    SessionEnd,
    Prompt,
    ToolCall,
    ToolResult,
    Approval,
    ConfigChange,
    PackageInstall,
    Other,
}

impl EventKind {
    pub fn as_str(self) -> &'static str {
        match self {
            EventKind::SessionStart => "session_start",
            EventKind::SessionEnd => "session_end",
            EventKind::Prompt => "prompt",
            EventKind::ToolCall => "tool_call",
            EventKind::ToolResult => "tool_result",
            EventKind::Approval => "approval",
            EventKind::ConfigChange => "config_change",
            EventKind::PackageInstall => "package_install",
            EventKind::Other => "other",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "session_start" => EventKind::SessionStart,
            "session_end" => EventKind::SessionEnd,
            "prompt" => EventKind::Prompt,
            "tool_call" => EventKind::ToolCall,
            "tool_result" => EventKind::ToolResult,
            "approval" => EventKind::Approval,
            "config_change" => EventKind::ConfigChange,
            "package_install" => EventKind::PackageInstall,
            _ => EventKind::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventSource {
    Hook,
    LogTail,
    Process,
    Shim,
}

impl EventSource {
    pub fn as_str(self) -> &'static str {
        match self {
            EventSource::Hook => "hook",
            EventSource::LogTail => "log_tail",
            EventSource::Process => "process",
            EventSource::Shim => "shim",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "log_tail" => EventSource::LogTail,
            "process" => EventSource::Process,
            "shim" => EventSource::Shim,
            _ => EventSource::Hook,
        }
    }
}
