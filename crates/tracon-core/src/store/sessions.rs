use anyhow::Result;
use rusqlite::{params, Connection};

use super::{
    collect_rows, row_to_event, row_to_event_lite, LiveSession, SessionSummary, Store,
    LATEST_CWD_SUBQUERY, LIVE_WINDOW_MINUTES,
};
use crate::clock;
use crate::event::AgentEvent;

impl Store {
    /// Every session, newest activity first, served from the rollup.
    pub fn sessions(&self) -> Result<Vec<SessionSummary>> {
        let conn = self.read_conn();
        let mut stmt = conn.prepare(
            "SELECT session_id, agent, cwd, started_at, last_at, event_count, command_count,
                    flagged_open_count, hook_tool_count, tail_tool_count, first_prompt
             FROM session_rollup
             ORDER BY last_at DESC
             LIMIT 200",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(SessionSummary {
                session_id: row.get(0)?,
                agent: row.get(1)?,
                cwd: row.get(2)?,
                started_at: row.get(3)?,
                last_at: row.get(4)?,
                event_count: row.get(5)?,
                command_count: row.get(6)?,
                flagged_count: row.get(7)?,
                hook_tool_count: row.get(8)?,
                tail_tool_count: row.get(9)?,
                first_prompt: row.get(10)?,
            })
        })?;
        collect_rows(rows)
    }

    /// The most recent `limit` events of a session, returned oldest-first.
    /// The window anchors at the tail so a long-running session shows its
    /// latest activity, not its first two thousand rows.
    pub fn events_for_session(&self, session_id: &str, limit: i64) -> Result<Vec<AgentEvent>> {
        let conn = self.read_conn();
        let mut stmt = conn.prepare(
            "SELECT * FROM (
               SELECT id, agent, session_id, ts, kind, source, cwd, tool_name, summary, flag, payload
               FROM events
               WHERE session_id = ?1
               ORDER BY ts DESC, id DESC
               LIMIT ?2
             ) ORDER BY ts ASC, id ASC",
        )?;
        let rows = stmt.query_map(params![session_id, limit], row_to_event)?;
        collect_rows(rows)
    }

    /// UI window over a session WITHOUT payloads: raw payloads dominate the
    /// row size by orders of magnitude and made 3-second polling visibly lag.
    /// The drill-down fetches a single payload on demand via event_payload.
    pub fn events_for_session_lite(&self, session_id: &str, limit: i64) -> Result<Vec<AgentEvent>> {
        let conn = self.read_conn();
        let mut stmt = conn.prepare(
            "SELECT * FROM (
               SELECT id, agent, session_id, ts, kind, source, cwd, tool_name, summary, flag
               FROM events
               WHERE session_id = ?1
               ORDER BY ts DESC, id DESC
               LIMIT ?2
             ) ORDER BY ts ASC, id ASC",
        )?;
        let rows = stmt.query_map(params![session_id, limit], row_to_event_lite)?;
        collect_rows(rows)
    }

    /// One event's full raw payload, for the drill-down view.
    pub fn event_payload(&self, id: i64) -> Result<Option<serde_json::Value>> {
        let conn = self.read_conn();
        let raw: Option<String> = conn
            .query_row(
                "SELECT payload FROM events WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .ok();
        Ok(raw.and_then(|s| serde_json::from_str(&s).ok()))
    }

    /// Global search across ALL recorded history (payload-free rows).
    /// Terms are ANDed; each may match summary, cwd, flag, tool, or agent.
    pub fn search_events(&self, query: &str, limit: i64) -> Result<Vec<AgentEvent>> {
        let terms: Vec<String> = query
            .split_whitespace()
            .take(5)
            .map(|t| format!("%{}%", escape_like(t)))
            .collect();
        if terms.is_empty() {
            return Ok(Vec::new());
        }

        let mut sql = String::from(
            "SELECT id, agent, session_id, ts, kind, source, cwd, tool_name, summary, flag
             FROM events WHERE ",
        );
        let clauses: Vec<String> = (1..=terms.len())
            .map(|p| {
                format!(
                    "(summary LIKE ?{p} ESCAPE '\\' OR cwd LIKE ?{p} ESCAPE '\\' \
                     OR flag LIKE ?{p} ESCAPE '\\' OR tool_name LIKE ?{p} ESCAPE '\\' \
                     OR agent LIKE ?{p} ESCAPE '\\')"
                )
            })
            .collect();
        sql.push_str(&clauses.join(" AND "));
        sql.push_str(&format!(" ORDER BY ts DESC, id DESC LIMIT {limit}"));

        let conn = self.read_conn();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(terms.iter()), row_to_event_lite)?;
        collect_rows(rows)
    }

    /// Sessions with events inside the cutoff window, newest first: the
    /// dashboard's live board. The prompt and last action look across the
    /// whole session (not just the window) so a long running task keeps its
    /// context line. Works identically for CLI and desktop agents; both
    /// arrive through the same hooks and transcript tailing.
    pub fn live_sessions(&self, cutoff_ts: &str, limit: i64) -> Result<Vec<LiveSession>> {
        let cutoff_ts = &clock::canonical_cutoff(cutoff_ts);
        let conn = self.read_conn();
        // INDEXED BY forces the ts range scan; left alone SQLite picks the
        // session index and walks the whole table for a five minute window.
        // Flags count open ones only, and the Task count is kind guarded so
        // a call's Pre and Post rows do not count as two subagents.
        let sql = format!(
            "SELECT session_id, agent,
                    {LATEST_CWD_SUBQUERY},
                    MAX(ts), COUNT(*),
                    SUM(CASE WHEN flag IS NOT NULL AND ack = 0 THEN 1 ELSE 0 END),
                    SUM(CASE WHEN kind = 'tool_call' AND tool_name = 'Task' THEN 1 ELSE 0 END),
                    (SELECT e2.summary FROM events e2
                     WHERE e2.session_id = events.session_id AND e2.kind = 'prompt'
                     ORDER BY e2.ts DESC, e2.id DESC LIMIT 1),
                    (SELECT e3.summary FROM events e3
                     WHERE e3.session_id = events.session_id AND e3.kind = 'tool_call'
                     ORDER BY e3.ts DESC, e3.id DESC LIMIT 1)
             FROM events INDEXED BY idx_events_ts
             WHERE ts >= ?1 AND agent != 'system'
             GROUP BY session_id, agent
             ORDER BY MAX(ts) DESC
             LIMIT ?2"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![cutoff_ts, limit], |row| {
            Ok(LiveSession {
                session_id: row.get(0)?,
                agent: row.get(1)?,
                cwd: row.get(2)?,
                last_ts: row.get(3)?,
                event_count: row.get(4)?,
                flagged_count: row.get::<_, Option<i64>>(5)?.unwrap_or(0),
                subagent_count: row.get::<_, Option<i64>>(6)?.unwrap_or(0),
                subagents: Vec::new(),
                last_prompt: row.get(7)?,
                last_action: row.get(8)?,
            })
        })?;
        let mut sessions: Vec<LiveSession> = collect_rows(rows)?;
        drop(stmt);
        for session in &mut sessions {
            if session.subagent_count > 0 {
                session.subagents = subagent_labels(&conn, &session.session_id, cutoff_ts);
            }
        }
        Ok(sessions)
    }

    /// live_sessions over the standard live window.
    pub fn live_sessions_recent(&self, limit: i64) -> Result<Vec<LiveSession>> {
        self.live_sessions(&clock::minutes_ago(LIVE_WINDOW_MINUTES), limit)
    }
}

/// Escape a search term for a LIKE pattern so `%` and `_` typed by the user
/// match literally instead of acting as wildcards.
fn escape_like(term: &str) -> String {
    term.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// Names of the subagents a session spawned recently, read from Task tool
/// call payloads. json_extract keeps the large payloads inside SQLite; only
/// short labels cross into Rust. Hook payloads nest the input under
/// tool_input, transcript rows under input; both paths are tried so tailed
/// sessions work too. The kind guard skips each call's result row.
fn subagent_labels(conn: &Connection, session_id: &str, cutoff_ts: &str) -> Vec<String> {
    let Ok(mut stmt) = conn.prepare(
        "SELECT COALESCE(
                  json_extract(payload, '$.tool_input.subagent_type'),
                  json_extract(payload, '$.input.subagent_type'),
                  json_extract(payload, '$.tool_input.description'),
                  json_extract(payload, '$.input.description'))
         FROM events
         WHERE session_id = ?1 AND kind = 'tool_call' AND tool_name = 'Task' AND ts >= ?2
         ORDER BY ts DESC, id DESC
         LIMIT 8",
    ) else {
        return Vec::new();
    };
    let Ok(rows) = stmt.query_map(params![session_id, cutoff_ts], |row| {
        row.get::<_, Option<String>>(0)
    }) else {
        return Vec::new();
    };

    let mut labels: Vec<String> = Vec::new();
    for label in rows.flatten().flatten() {
        if !labels.contains(&label) {
            labels.push(label);
        }
        if labels.len() >= 3 {
            break;
        }
    }
    labels
}
