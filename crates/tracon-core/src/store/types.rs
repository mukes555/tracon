use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct SessionSummary {
    pub session_id: String,
    pub agent: String,
    pub cwd: Option<String>,
    pub started_at: String,
    pub last_at: String,
    pub event_count: i64,
    pub command_count: i64,
    pub flagged_count: i64,
    /// Tool calls captured live via hooks vs recovered from the transcript.
    /// Both being nonzero means hooks stopped mid-session: a capture gap
    /// worth flagging to the user (hooks disabled, or Tracon was closed).
    pub hook_tool_count: i64,
    pub tail_tool_count: i64,
    pub first_prompt: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CaptureCount {
    pub agent: String,
    pub source: String,
    pub count: i64,
    pub last_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Stats {
    pub session_count: i64,
    pub event_count: i64,
    pub command_count: i64,
    pub package_count: i64,
    /// Open (untriaged) flags; acknowledged ones move to acked_count.
    pub flagged_count: i64,
    pub acked_count: i64,
    pub last_event_at: Option<String>,
    pub sessions_today: i64,
    pub commands_today: i64,
    pub packages_today: i64,
}

/// How long a session counts as live after its last event. One constant,
/// shared by the live queries and the change token, and deliberately not
/// repeated in UI copy.
pub const LIVE_WINDOW_MINUTES: i64 = 5;

/// Cheap change signal for UI polling: new events bump max_id, triage moves
/// flags between the open and acked counts, and live_sessions changes when
/// a session enters or ages out of the live window, so decay itself is a
/// token change.
#[derive(Debug, Serialize, PartialEq)]
pub struct ChangeToken {
    pub max_id: i64,
    pub open_flags: i64,
    pub acked_flags: i64,
    pub live_sessions: i64,
}

#[derive(Debug, Serialize)]
pub struct DayCount {
    pub day: String,
    pub events: i64,
    pub flagged: i64,
}

/// One row of the dashboard's live board: a session with recent activity,
/// carrying enough context to understand what the agent is doing right now.
#[derive(Debug, Serialize)]
pub struct LiveSession {
    pub session_id: String,
    pub agent: String,
    pub cwd: Option<String>,
    pub last_ts: String,
    /// Events and flags inside the live window, not session totals.
    pub event_count: i64,
    pub flagged_count: i64,
    /// Task tool calls in the window: the agent spawning subagents.
    pub subagent_count: i64,
    /// Names of the most recent subagents (subagent_type from the Task
    /// payload, or its description), deduplicated, newest first, max 3.
    pub subagents: Vec<String>,
    pub last_prompt: Option<String>,
    pub last_action: Option<String>,
}
