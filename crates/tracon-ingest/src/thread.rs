//! On-demand conversation reader. The timeline stores actions; the actual
//! chat (including assistant prose we deliberately don't persist) is read
//! straight from the agent's own transcript on disk, read-only, when the
//! user asks to see the thread behind an event.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

use crate::tailer;

const MAX_MESSAGES: usize = 500;
const MAX_TEXT_CHARS: usize = 6000;

#[derive(Debug, Serialize)]
pub struct ThreadMessage {
    pub role: String,
    pub ts: String,
    pub text: String,
}

/// Locate and parse the conversation for a session, newest agents first.
/// Unknown sessions (hook-only agents, system pseudo-sessions) return empty.
pub fn read_thread(session_id: &str) -> Vec<ThreadMessage> {
    if let Some(path) = find_claude_transcript(session_id) {
        return trim(parse_claude_thread(&path));
    }
    if let Some(path) = find_codex_rollout(session_id) {
        return trim(parse_codex_thread(&path));
    }
    Vec::new()
}

fn trim(mut msgs: Vec<ThreadMessage>) -> Vec<ThreadMessage> {
    if msgs.len() > MAX_MESSAGES {
        msgs.drain(..msgs.len() - MAX_MESSAGES);
    }
    msgs
}

fn find_claude_transcript(session_id: &str) -> Option<PathBuf> {
    let root = tailer::claude_projects_dir()?;
    let wanted = format!("{session_id}.jsonl");
    for project in std::fs::read_dir(root).ok()?.flatten() {
        let candidate = project.path().join(&wanted);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn find_codex_rollout(session_id: &str) -> Option<PathBuf> {
    let root = tailer::codex_sessions_dir()?;
    let suffix = format!("{session_id}.jsonl");
    find_by_suffix(&root, &suffix, 0)
}

fn find_by_suffix(dir: &Path, suffix: &str, depth: usize) -> Option<PathBuf> {
    if depth > 5 {
        return None;
    }
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_by_suffix(&path, suffix, depth + 1) {
                return Some(found);
            }
        } else if path.to_string_lossy().ends_with(suffix) {
            return Some(path);
        }
    }
    None
}

pub fn parse_claude_thread(path: &Path) -> Vec<ThreadMessage> {
    // Chat rows must carry these role markers somewhere; most rows are tool
    // calls and results, and skipping their JSON parse is the bulk of the win.
    collect_messages(
        path,
        |line| line.contains("\"user\"") || line.contains("\"assistant\""),
        claude_message,
    )
}

pub fn parse_codex_thread(path: &Path) -> Vec<ThreadMessage> {
    collect_messages(path, |line| line.contains("response_item"), codex_message)
}

/// Stream the transcript line by line (files reach hundreds of MB, so no
/// whole-file read), parsing only lines that pass the cheap prefilter, and
/// keep the tail bounded while scanning.
fn collect_messages(
    path: &Path,
    could_be_chat: fn(&str) -> bool,
    parse: fn(&str) -> Option<ThreadMessage>,
) -> Vec<ThreadMessage> {
    let Ok(file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let mut msgs = Vec::new();
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        if !could_be_chat(&line) {
            continue;
        }
        if let Some(msg) = parse(&line) {
            msgs.push(msg);
        }
        if msgs.len() >= MAX_MESSAGES * 2 {
            msgs.drain(..MAX_MESSAGES);
        }
    }
    msgs
}

fn claude_message(line: &str) -> Option<ThreadMessage> {
    let row: Value = serde_json::from_str(line).ok()?;
    let role = row.get("type")?.as_str()?;
    if role != "user" && role != "assistant" {
        return None;
    }
    let text = message_text(row.get("message")?)?;
    Some(ThreadMessage {
        role: role.to_string(),
        ts: timestamp_of(&row),
        text,
    })
}

fn codex_message(line: &str) -> Option<ThreadMessage> {
    let row: Value = serde_json::from_str(line).ok()?;
    if row.get("type")?.as_str()? != "response_item" {
        return None;
    }
    let payload = row.get("payload")?;
    if payload.get("type")?.as_str()? != "message" {
        return None;
    }
    let role = payload.get("role")?.as_str()?;
    let text = joined_text_blocks(payload.get("content")?)?;
    Some(ThreadMessage {
        role: role.to_string(),
        ts: timestamp_of(&row),
        text,
    })
}

fn timestamp_of(row: &Value) -> String {
    row.get("timestamp")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// Claude message content is either a plain string or blocks; only text
/// blocks count (tool_use/tool_result stay in the timeline where they live).
fn message_text(message: &Value) -> Option<String> {
    let content = message.get("content")?;
    if let Some(text) = content.as_str() {
        return clamp_non_empty(text);
    }
    joined_text_blocks(content)
}

fn joined_text_blocks(content: &Value) -> Option<String> {
    let joined = content
        .as_array()?
        .iter()
        .filter(|b| {
            matches!(
                b.get("type").and_then(Value::as_str),
                Some("text") | Some("input_text") | Some("output_text")
            )
        })
        .filter_map(|b| b.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n");
    clamp_non_empty(&joined)
}

fn clamp_non_empty(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.chars().count() <= MAX_TEXT_CHARS {
        return Some(trimmed.to_string());
    }
    let cut: String = trimmed.chars().take(MAX_TEXT_CHARS).collect();
    Some(format!("{cut}\n[...]"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str, content: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tracon-thread-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn claude_thread_keeps_prose_and_skips_tool_rows() {
        let lines = [
            r#"{"type":"user","sessionId":"s","timestamp":"t1","message":{"content":"fix the bug"}}"#,
            r#"{"type":"assistant","sessionId":"s","timestamp":"t2","message":{"content":[{"type":"text","text":"On it."},{"type":"tool_use","id":"x","name":"Bash","input":{}}]}}"#,
            r#"{"type":"user","sessionId":"s","timestamp":"t3","message":{"content":[{"type":"tool_result","tool_use_id":"x","content":"ok"}]}}"#,
        ]
        .join("\n");
        let path = temp_file("claude.jsonl", &lines);

        let thread = parse_claude_thread(&path);
        assert_eq!(thread.len(), 2);
        assert_eq!(thread[0].role, "user");
        assert_eq!(thread[0].text, "fix the bug");
        assert_eq!(thread[1].role, "assistant");
        assert_eq!(thread[1].text, "On it.");
    }

    #[test]
    fn codex_thread_reads_message_items_only() {
        let lines = [
            r#"{"type":"response_item","timestamp":"t1","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"add tests"}]}}"#,
            r#"{"type":"response_item","timestamp":"t2","payload":{"type":"function_call","name":"shell","call_id":"c"}}"#,
            r#"{"type":"response_item","timestamp":"t3","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Done."}]}}"#,
        ]
        .join("\n");
        let path = temp_file("codex.jsonl", &lines);

        let thread = parse_codex_thread(&path);
        assert_eq!(thread.len(), 2);
        assert_eq!(thread[1].role, "assistant");
        assert_eq!(thread[1].text, "Done.");
    }
}
