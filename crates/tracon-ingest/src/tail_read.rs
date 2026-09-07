//! Incremental, bounded reading of one tailed log file.
//!
//! Transcript files are written by other programs and can contain anything:
//! partial lines mid-write, invalid UTF-8, or a single multi-megabyte line.
//! None of that may stall the tailer or make it re-read the same bytes.

use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use tracon_core::store::Store;

use crate::sink::insert_event;
use crate::tailer::LineParser;

pub const MAX_LINE_BYTES: usize = 1024 * 1024;
const READ_CHUNK: u64 = 64 * 1024;

enum LineRead {
    Complete(u64),
    /// Longer than MAX_LINE_BYTES: the bytes were consumed and dropped.
    Oversized(u64),
    /// EOF without a newline (a line still being written) or a read error.
    Incomplete,
}

/// Read everything new since the stored offset, parse complete lines, and
/// advance the offset only past the last full line so a partially written
/// line is picked up whole on the next change. The offset also stays put when
/// any insert failed, so a database hiccup never loses events.
pub fn process_file(store: &Store, path: &Path, parser: LineParser) {
    if store.capture_paused() {
        return;
    }
    let path_key = path.to_string_lossy().to_string();
    let mut offset = store.tail_offset(&path_key).unwrap_or(0);

    let Ok(meta) = path.metadata() else { return };
    if meta.len() < offset {
        // The file was truncated or replaced; start over.
        offset = 0;
    }
    if meta.len() == offset {
        return;
    }

    let Ok(mut file) = File::open(path) else {
        return;
    };
    if file.seek(SeekFrom::Start(offset)).is_err() {
        return;
    }

    let mut reader = BufReader::new(file);
    let mut consumed = 0u64;
    let mut all_inserts_ok = true;
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match read_line_bounded(&mut reader, &mut buf) {
            LineRead::Complete(len) => {
                let text = String::from_utf8_lossy(strip_line_ending(&buf));
                for event in parser(&text, path) {
                    if insert_event(store, event).is_err() {
                        all_inserts_ok = false;
                    }
                }
                consumed += len;
            }
            LineRead::Oversized(len) => consumed += len,
            LineRead::Incomplete => break,
        }
    }

    if consumed == 0 || !all_inserts_ok {
        return;
    }
    let _ = store.set_tail_offset(&path_key, offset + consumed);
}

/// Read one line in bounded chunks. Once a line passes MAX_LINE_BYTES its
/// bytes are discarded as they stream in, so memory stays flat no matter how
/// long the line is, and the returned length still counts every byte.
fn read_line_bounded<R: BufRead>(reader: &mut R, buf: &mut Vec<u8>) -> LineRead {
    let mut total = 0u64;
    let mut oversized = false;
    loop {
        let Ok(n) = (&mut *reader).take(READ_CHUNK).read_until(b'\n', buf) else {
            return LineRead::Incomplete;
        };
        if n == 0 {
            return LineRead::Incomplete;
        }
        total += n as u64;

        let line_ended = buf.last() == Some(&b'\n');
        if buf.len() > MAX_LINE_BYTES {
            oversized = true;
            buf.clear();
        }
        if !line_ended {
            continue;
        }
        return if oversized {
            LineRead::Oversized(total)
        } else {
            LineRead::Complete(total)
        };
    }
}

fn strip_line_ending(line: &[u8]) -> &[u8] {
    let without_lf = line.strip_suffix(b"\n").unwrap_or(line);
    without_lf.strip_suffix(b"\r").unwrap_or(without_lf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tailer::claude_parser;
    use crate::test_support::temp_dir;

    fn tool_use_line(id: &str, command: &str) -> String {
        serde_json::json!({
            "type": "assistant",
            "sessionId": "s1",
            "timestamp": "2026-08-29T12:00:00Z",
            "message": {"content": [
                {"type": "tool_use", "id": id, "name": "Bash", "input": {"command": command}}
            ]}
        })
        .to_string()
    }

    fn offset_of(store: &Store, file: &Path) -> u64 {
        store.tail_offset(&file.to_string_lossy()).unwrap()
    }

    #[test]
    fn processes_appends_incrementally_without_reprocessing() {
        let store = Store::open_in_memory().unwrap();
        let dir = temp_dir("tail-append");
        let file = dir.join("session.jsonl");

        std::fs::write(&file, format!("{}\n", tool_use_line("t1", "ls"))).unwrap();
        process_file(&store, &file, claude_parser);
        assert_eq!(store.stats().unwrap().event_count, 1);

        // Reprocessing with no new content is a no-op.
        process_file(&store, &file, claude_parser);
        assert_eq!(store.stats().unwrap().event_count, 1);

        // Appending only processes the new line; a partial line waits.
        let mut content = std::fs::read_to_string(&file).unwrap();
        content.push_str(&format!(
            "{}\n",
            tool_use_line("t2", "npm install left-pad")
        ));
        content.push_str("{\"type\":\"assistant\",\"sessionId\":\"s1\"");
        std::fs::write(&file, &content).unwrap();
        process_file(&store, &file, claude_parser);

        // t2 yields a tool_call plus a package_install.
        assert_eq!(store.stats().unwrap().event_count, 3);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn invalid_utf8_line_is_decoded_lossily_and_offset_advances() {
        let store = Store::open_in_memory().unwrap();
        let dir = temp_dir("tail-utf8");
        let file = dir.join("session.jsonl");

        let mut bytes = tool_use_line("t1", "echo ZZZ").into_bytes();
        let marker = bytes.windows(3).position(|w| w == b"ZZZ").unwrap();
        bytes[marker..marker + 3].copy_from_slice(&[0xff, 0xfe, 0x41]);
        bytes.push(b'\n');
        bytes.extend_from_slice(tool_use_line("t2", "ls").as_bytes());
        bytes.push(b'\n');
        std::fs::write(&file, &bytes).unwrap();

        process_file(&store, &file, claude_parser);
        assert_eq!(store.stats().unwrap().event_count, 2);
        assert_eq!(offset_of(&store, &file), bytes.len() as u64);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn oversized_line_is_skipped_but_consumed() {
        let store = Store::open_in_memory().unwrap();
        let dir = temp_dir("tail-oversized");
        let file = dir.join("session.jsonl");

        let huge = format!("{{\"junk\":\"{}\"}}\n", "a".repeat(MAX_LINE_BYTES + 10));
        let content = format!("{huge}{}\n", tool_use_line("t1", "ls"));
        std::fs::write(&file, &content).unwrap();

        process_file(&store, &file, claude_parser);
        assert_eq!(store.stats().unwrap().event_count, 1);
        assert_eq!(offset_of(&store, &file), content.len() as u64);
        std::fs::remove_dir_all(&dir).ok();
    }
}
