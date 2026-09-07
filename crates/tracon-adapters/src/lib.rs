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

const SESSION_ID_MAX_CHARS: usize = 128;

/// Session ids come from untrusted places (hook payloads, log filenames) and
/// end up in SQL parameters, URLs, and export filenames. Keeping them to a
/// plain character set means a crafted id like "../../etc" can never become
/// a path or escape a query, whatever a downstream consumer does with it.
pub fn sanitize_session_id(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .take(SESSION_ID_MAX_CHARS)
        .map(|c| {
            let allowed = c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-');
            if allowed {
                c
            } else {
                '_'
            }
        })
        .collect();
    if cleaned.is_empty() {
        return "unknown".to_string();
    }
    cleaned
}

/// Cut a summary down to `max` characters (not bytes, so multibyte text is
/// safe) and mark the cut with an ellipsis.
pub fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let cut: String = s.chars().take(max).collect();
    format!("{cut}...")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_neutralizes_path_traversal() {
        assert_eq!(sanitize_session_id("../../etc"), ".._.._etc");
        assert_eq!(sanitize_session_id("a/b\\c d"), "a_b_c_d");
    }

    #[test]
    fn sanitize_keeps_normal_ids_and_caps_length() {
        let uuid = "8f14e45f-ceea-4a67-a1b2-c3d4e5f60718";
        assert_eq!(sanitize_session_id(uuid), uuid);
        assert_eq!(sanitize_session_id("conv_1.2"), "conv_1.2");
        let long = "x".repeat(300);
        assert_eq!(sanitize_session_id(&long).len(), SESSION_ID_MAX_CHARS);
    }

    #[test]
    fn sanitize_falls_back_to_unknown_when_empty() {
        assert_eq!(sanitize_session_id(""), "unknown");
    }

    #[test]
    fn truncate_counts_chars_not_bytes() {
        assert_eq!(truncate("héllo", 10), "héllo");
        assert_eq!(truncate("héllo wörld", 5), "héllo...");
    }
}
