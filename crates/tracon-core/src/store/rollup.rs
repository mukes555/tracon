//! Per-session aggregate maintained alongside the events table, so the
//! sessions list and the headline stats read a few hundred small rows
//! instead of grouping every event ever recorded.
//!
//! The rollup is updated inside the same transaction as each insert,
//! refreshed on triage (ack, backfilled flags), and rebuilt from scratch
//! whenever events are deleted in bulk.

use anyhow::Result;
use rusqlite::{params, Connection, Transaction};

use super::{COMMAND_COUNT_CASE, LATEST_CWD_SUBQUERY};
use crate::event::{AgentEvent, EventKind, EventSource};

/// cwd_ts and first_prompt_ts record which event supplied the value, so
/// an out-of-order insert (a transcript backfilled after live hooks) can
/// tell whether it is newer than the cwd we already have.
pub(super) const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS session_rollup (
  session_id TEXT PRIMARY KEY,
  agent TEXT NOT NULL,
  cwd TEXT,
  cwd_ts TEXT,
  started_at TEXT NOT NULL,
  last_at TEXT NOT NULL,
  event_count INTEGER NOT NULL DEFAULT 0,
  command_count INTEGER NOT NULL DEFAULT 0,
  flagged_open_count INTEGER NOT NULL DEFAULT 0,
  hook_tool_count INTEGER NOT NULL DEFAULT 0,
  tail_tool_count INTEGER NOT NULL DEFAULT 0,
  first_prompt TEXT,
  first_prompt_ts TEXT
);
CREATE INDEX IF NOT EXISTS idx_rollup_last_at ON session_rollup(last_at);
";

/// Fold one freshly inserted event into its session's row. `ts` is the
/// normalized timestamp that was actually stored, not the raw event.ts.
pub(super) fn apply_event(tx: &Transaction<'_>, event: &AgentEvent, ts: &str) -> Result<()> {
    let is_tool_call = event.kind == EventKind::ToolCall;
    let is_command =
        is_tool_call && matches!(event.tool_name.as_deref(), Some("Bash") | Some("shell"));
    let is_hook_tool = is_tool_call && event.source == EventSource::Hook;
    let is_tail_tool = is_tool_call && event.source == EventSource::LogTail;
    let is_prompt = event.kind == EventKind::Prompt;
    let cwd_ts = event.cwd.as_ref().map(|_| ts);
    let prompt_ts = if is_prompt { Some(ts) } else { None };
    let prompt_text = if is_prompt {
        event.summary.as_deref()
    } else {
        None
    };

    // The latest cwd wins ties on equal ts (the new row has the higher id);
    // the first prompt keeps the existing one on ties, mirroring the
    // ORDER BY ts, id of the original per-session subqueries.
    tx.execute(
        "INSERT INTO session_rollup
           (session_id, agent, cwd, cwd_ts, started_at, last_at, event_count, command_count,
            flagged_open_count, hook_tool_count, tail_tool_count, first_prompt, first_prompt_ts)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5, 1, ?6, ?7, ?8, ?9, ?10, ?11)
         ON CONFLICT(session_id) DO UPDATE SET
           cwd = CASE WHEN excluded.cwd_ts IS NOT NULL
                       AND (cwd_ts IS NULL OR excluded.cwd_ts >= cwd_ts)
                      THEN excluded.cwd ELSE cwd END,
           cwd_ts = CASE WHEN excluded.cwd_ts IS NOT NULL
                          AND (cwd_ts IS NULL OR excluded.cwd_ts >= cwd_ts)
                         THEN excluded.cwd_ts ELSE cwd_ts END,
           started_at = MIN(started_at, excluded.started_at),
           last_at = MAX(last_at, excluded.last_at),
           event_count = event_count + 1,
           command_count = command_count + excluded.command_count,
           flagged_open_count = flagged_open_count + excluded.flagged_open_count,
           hook_tool_count = hook_tool_count + excluded.hook_tool_count,
           tail_tool_count = tail_tool_count + excluded.tail_tool_count,
           first_prompt = CASE WHEN excluded.first_prompt_ts IS NOT NULL
                                AND (first_prompt_ts IS NULL
                                     OR excluded.first_prompt_ts < first_prompt_ts)
                               THEN excluded.first_prompt ELSE first_prompt END,
           first_prompt_ts = CASE WHEN excluded.first_prompt_ts IS NOT NULL
                                   AND (first_prompt_ts IS NULL
                                        OR excluded.first_prompt_ts < first_prompt_ts)
                                  THEN excluded.first_prompt_ts ELSE first_prompt_ts END",
        params![
            event.session_id,
            event.agent,
            event.cwd,
            cwd_ts,
            ts,
            is_command as i64,
            event.flag.is_some() as i64,
            is_hook_tool as i64,
            is_tail_tool as i64,
            prompt_text,
            prompt_ts,
        ],
    )?;
    Ok(())
}

/// Recount open flags for the sessions owning the given event ids, after
/// an ack or a backfilled flag changed them. Counting is cheaper and safer
/// than tracking deltas across ack/reopen/flag transitions.
pub(super) fn refresh_flag_counts(tx: &Transaction<'_>, event_ids: &[i64]) -> Result<()> {
    let mut lookup = tx.prepare("SELECT session_id FROM events WHERE id = ?1")?;
    let mut refresh = tx.prepare(
        "UPDATE session_rollup
         SET flagged_open_count = (SELECT COUNT(*) FROM events
                                   WHERE events.session_id = session_rollup.session_id
                                     AND flag IS NOT NULL AND ack = 0)
         WHERE session_id = ?1",
    )?;
    let mut done: Vec<String> = Vec::new();
    for id in event_ids {
        let Ok(session_id) = lookup.query_row(params![id], |row| row.get::<_, String>(0)) else {
            continue;
        };
        if done.contains(&session_id) {
            continue;
        }
        refresh.execute(params![session_id])?;
        done.push(session_id);
    }
    Ok(())
}

/// Recompute every session row from the events table. The agent is the one
/// that opened the session; cwd and first prompt follow the same ordering
/// rules as the incremental path.
pub(super) fn rebuild(conn: &Connection) -> Result<()> {
    conn.execute("DELETE FROM session_rollup", [])?;
    let sql = format!(
        "INSERT INTO session_rollup
           (session_id, agent, cwd, cwd_ts, started_at, last_at, event_count, command_count,
            flagged_open_count, hook_tool_count, tail_tool_count, first_prompt, first_prompt_ts)
         SELECT session_id,
                (SELECT e0.agent FROM events e0 WHERE e0.session_id = events.session_id
                 ORDER BY e0.ts ASC, e0.id ASC LIMIT 1),
                {LATEST_CWD_SUBQUERY},
                (SELECT e1.ts FROM events e1
                 WHERE e1.session_id = events.session_id AND e1.cwd IS NOT NULL
                 ORDER BY e1.ts DESC, e1.id DESC LIMIT 1),
                MIN(ts), MAX(ts), COUNT(*),
                SUM({COMMAND_COUNT_CASE}),
                SUM(CASE WHEN flag IS NOT NULL AND ack = 0 THEN 1 ELSE 0 END),
                SUM(CASE WHEN kind = 'tool_call' AND source = 'hook' THEN 1 ELSE 0 END),
                SUM(CASE WHEN kind = 'tool_call' AND source = 'log_tail' THEN 1 ELSE 0 END),
                (SELECT e2.summary FROM events e2
                 WHERE e2.session_id = events.session_id AND e2.kind = 'prompt'
                 ORDER BY e2.ts ASC, e2.id ASC LIMIT 1),
                (SELECT e2.ts FROM events e2
                 WHERE e2.session_id = events.session_id AND e2.kind = 'prompt'
                 ORDER BY e2.ts ASC, e2.id ASC LIMIT 1)
         FROM events
         GROUP BY session_id"
    );
    conn.execute(&sql, [])?;
    Ok(())
}
