pub mod claude;
pub mod claude_transcript;
pub mod codex;
pub mod cursor;
pub mod danger;
pub mod gemini;
pub mod packages;

use serde_json::Value;
use tracon_core::event::AgentEvent;

/// Route an incoming hook payload to the right agent adapter by shape:
/// Claude Code identifies sessions with session_id, Cursor with
/// conversation_id. Unknown shapes yield nothing, never an error.
pub fn events_from_any_hook_payload(payload: &Value) -> Vec<AgentEvent> {
    if payload.get("conversation_id").is_some() {
        return cursor::events_from_hook_payload(payload);
    }
    claude::events_from_hook_payload(payload)
}
