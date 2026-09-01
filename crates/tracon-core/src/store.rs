use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard};

use anyhow::Result;
use rusqlite::{params, Connection, OpenFlags, Row};
use serde::Serialize;

use crate::event::{AgentEvent, EventKind, EventSource};

const READ_POOL_SIZE: usize = 3;

/// Local SQLite event store. All Tracon data lives in one file on the user's machine.
///
/// Writes go through one connection; reads go through a small read-only pool
/// so UI queries never wait behind background writers (tailers, imports,
/// workers). WAL mode makes that concurrency safe. In-memory stores (tests)
/// have no pool and fall back to the writer connection.
pub struct Store {
    conn: Mutex<Connection>,
    readers: Vec<Mutex<Connection>>,
    next_reader: AtomicUsize,
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS events (
  id INTEGER PRIMARY KEY,
  agent TEXT NOT NULL,
  session_id TEXT NOT NULL,
  ts TEXT NOT NULL,
  kind TEXT NOT NULL,
  source TEXT NOT NULL,
  cwd TEXT,
  tool_name TEXT,
  summary TEXT,
  flag TEXT,
  intel_checked INTEGER NOT NULL DEFAULT 0,
  payload TEXT NOT NULL,
  dedupe_key TEXT UNIQUE
);
CREATE INDEX IF NOT EXISTS idx_events_session ON events(session_id, ts);
CREATE INDEX IF NOT EXISTS idx_events_ts ON events(ts);
CREATE INDEX IF NOT EXISTS idx_events_prompt ON events(session_id, ts) WHERE kind = 'prompt';
CREATE INDEX IF NOT EXISTS idx_events_kind_ts ON events(kind, ts);
CREATE TABLE IF NOT EXISTS tail_offsets (
  path TEXT PRIMARY KEY,
  offset INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
";

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

/// Cheap change signal for UI polling: new events bump max_id, triage moves
/// flags between the open and acked counts, flag backfill bumps open_flags.
#[derive(Debug, Serialize, PartialEq)]
pub struct ChangeToken {
    pub max_id: i64,
    pub open_flags: i64,
    pub acked_flags: i64,
}

#[derive(Debug, Serialize)]
pub struct DayCount {
    pub day: String,
    pub events: i64,
    pub flagged: i64,
}

#[derive(Debug, Serialize)]
pub struct LiveAgent {
    pub agent: String,
    pub last_ts: String,
    pub cwd: Option<String>,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        let mut store = Self::from_conn(Connection::open(path)?)?;
        let read_flags = OpenFlags::SQLITE_OPEN_READ_ONLY;
        for _ in 0..READ_POOL_SIZE {
            let reader = Connection::open_with_flags(path, read_flags)?;
            reader.busy_timeout(std::time::Duration::from_secs(5))?;
            store.readers.push(Mutex::new(reader));
        }
        Ok(store)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::from_conn(Connection::open_in_memory()?)
    }

    fn from_conn(conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.execute_batch(SCHEMA)?;
        migrate(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
            readers: Vec::new(),
            next_reader: AtomicUsize::new(0),
        })
    }

    fn read_conn(&self) -> MutexGuard<'_, Connection> {
        if self.readers.is_empty() {
            return self.conn.lock().expect("store mutex poisoned");
        }
        let i = self.next_reader.fetch_add(1, Ordering::Relaxed) % self.readers.len();
        self.readers[i].lock().expect("store mutex poisoned")
    }

    /// Insert an event. Returns false when the dedupe key says we already have it.
    pub fn insert(&self, event: &AgentEvent) -> Result<bool> {
        let payload = serde_json::to_string(&event.payload)?;
        let conn = self.conn.lock().expect("store mutex poisoned");
        let inserted = conn.execute(
            "INSERT OR IGNORE INTO events
               (agent, session_id, ts, kind, source, cwd, tool_name, summary, flag, payload, dedupe_key)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                event.agent,
                event.session_id,
                event.ts,
                event.kind.as_str(),
                event.source.as_str(),
                event.cwd,
                event.tool_name,
                event.summary,
                event.flag,
                payload,
                event.dedupe_key,
            ],
        )?;
        Ok(inserted > 0)
    }

    pub fn sessions(&self) -> Result<Vec<SessionSummary>> {
        let conn = self.read_conn();
        let mut stmt = conn.prepare(
            "SELECT session_id, agent, MAX(COALESCE(cwd, '')), MIN(ts), MAX(ts), COUNT(*),
                    SUM(CASE WHEN kind = 'tool_call' AND tool_name IN ('Bash', 'shell') THEN 1 ELSE 0 END),
                    SUM(CASE WHEN flag IS NOT NULL THEN 1 ELSE 0 END),
                    SUM(CASE WHEN kind = 'tool_call' AND source = 'hook' THEN 1 ELSE 0 END),
                    SUM(CASE WHEN kind = 'tool_call' AND source = 'log_tail' THEN 1 ELSE 0 END),
                    (SELECT e2.summary FROM events e2
                     WHERE e2.session_id = events.session_id AND e2.kind = 'prompt'
                     ORDER BY e2.ts ASC, e2.id ASC LIMIT 1)
             FROM events
             GROUP BY session_id, agent
             ORDER BY MAX(ts) DESC
             LIMIT 200",
        )?;
        let rows = stmt.query_map([], |row| {
            let cwd: String = row.get(2)?;
            Ok(SessionSummary {
                session_id: row.get(0)?,
                agent: row.get(1)?,
                cwd: if cwd.is_empty() { None } else { Some(cwd) },
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
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
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
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
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
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    /// Every package install across sessions, newest first (payload-free).
    pub fn package_events(&self, limit: i64) -> Result<Vec<AgentEvent>> {
        let conn = self.read_conn();
        let mut stmt = conn.prepare(
            "SELECT id, agent, session_id, ts, kind, source, cwd, tool_name, summary, flag
             FROM events
             WHERE kind = 'package_install'
             ORDER BY ts DESC, id DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], row_to_event_lite)?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    /// Flagged events across sessions, newest first (payload-free).
    /// `acked` selects the triage bucket: open flags or acknowledged ones.
    pub fn flagged_events(&self, limit: i64, acked: bool) -> Result<Vec<AgentEvent>> {
        let conn = self.read_conn();
        let mut stmt = conn.prepare(
            "SELECT id, agent, session_id, ts, kind, source, cwd, tool_name, summary, flag
             FROM events
             WHERE flag IS NOT NULL AND ack = ?2
             ORDER BY ts DESC, id DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit, acked as i64], row_to_event_lite)?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    /// Global search across ALL recorded history (payload-free rows).
    /// Terms are ANDed; each may match summary, cwd, flag, tool, or agent.
    pub fn search_events(&self, query: &str, limit: i64) -> Result<Vec<AgentEvent>> {
        let terms: Vec<String> = query
            .split_whitespace()
            .take(5)
            .map(|t| {
                format!(
                    "%{}%",
                    t.replace('\\', "\\\\")
                        .replace('%', "\\%")
                        .replace('_', "\\_")
                )
            })
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
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    /// Mark a flagged event reviewed (or reopen it).
    pub fn set_ack(&self, id: i64, acked: bool) -> Result<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE events SET ack = ?2 WHERE id = ?1",
            params![id, acked as i64],
        )?;
        Ok(())
    }

    /// Agents with events inside the cutoff window, with their latest activity.
    pub fn live_agents(&self, cutoff_ts: &str) -> Result<Vec<LiveAgent>> {
        let conn = self.read_conn();
        let mut stmt = conn.prepare(
            "SELECT agent, MAX(ts),
                    (SELECT cwd FROM events e2
                     WHERE e2.agent = events.agent AND e2.ts >= ?1 AND e2.cwd IS NOT NULL
                     ORDER BY e2.ts DESC LIMIT 1)
             FROM events
             WHERE ts >= ?1 AND agent != 'system'
             GROUP BY agent
             ORDER BY MAX(ts) DESC",
        )?;
        let rows = stmt.query_map(params![cutoff_ts], |row| {
            Ok(LiveAgent {
                agent: row.get(0)?,
                last_ts: row.get(1)?,
                cwd: row.get(2)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    /// live_agents with the cutoff computed here (now minus `minutes`).
    pub fn live_agents_recent(&self, minutes: i64) -> Result<Vec<LiveAgent>> {
        let cutoff = (time::OffsetDateTime::now_utc() - time::Duration::minutes(minutes))
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default();
        self.live_agents(&cutoff)
    }

    /// True while the user has paused capture from the tray.
    pub fn capture_paused(&self) -> bool {
        self.setting("capture_paused")
            .ok()
            .flatten()
            .is_some_and(|v| v == "true")
    }

    /// Highest event id in the store; the notifier baselines here at startup
    /// so historical flags never fire notifications.
    pub fn max_event_id(&self) -> Result<i64> {
        let conn = self.read_conn();
        let id = conn.query_row("SELECT COALESCE(MAX(id), 0) FROM events", [], |row| {
            row.get(0)
        })?;
        Ok(id)
    }

    /// Flagged events newer than the given id, oldest first (payload-free).
    pub fn flagged_events_after(&self, after_id: i64, limit: i64) -> Result<Vec<AgentEvent>> {
        let conn = self.read_conn();
        let mut stmt = conn.prepare(
            "SELECT id, agent, session_id, ts, kind, source, cwd, tool_name, summary, flag
             FROM events
             WHERE flag IS NOT NULL AND id > ?1
             ORDER BY id ASC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![after_id, limit], row_to_event_lite)?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    /// Agents (excluding the system pseudo-agent) with any event at or after
    /// the cutoff: "who was active on this machine just now".
    pub fn agents_active_since(&self, cutoff_ts: &str) -> Result<Vec<String>> {
        let conn = self.read_conn();
        let mut stmt = conn.prepare(
            "SELECT DISTINCT agent FROM events WHERE ts >= ?1 AND agent != 'system' ORDER BY agent",
        )?;
        let rows = stmt.query_map(params![cutoff_ts], |row| row.get(0))?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
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

    /// How far into a tailed log file we have already read.
    pub fn tail_offset(&self, path: &str) -> Result<u64> {
        let conn = self.read_conn();
        let offset = conn
            .query_row(
                "SELECT offset FROM tail_offsets WHERE path = ?1",
                params![path],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(0);
        Ok(offset.max(0) as u64)
    }

    pub fn set_tail_offset(&self, path: &str, offset: u64) -> Result<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO tail_offsets (path, offset) VALUES (?1, ?2)
             ON CONFLICT(path) DO UPDATE SET offset = excluded.offset",
            params![path, offset as i64],
        )?;
        Ok(())
    }

    /// Bash tool calls that have never been assessed for danger flags
    /// (rows recorded by builds that predate flag support).
    pub fn unassessed_bash_events(&self, limit: i64) -> Result<Vec<(i64, String)>> {
        let conn = self.read_conn();
        let mut stmt = conn.prepare(
            "SELECT id, payload FROM events
             WHERE kind = 'tool_call' AND tool_name = 'Bash' AND flag IS NULL
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn set_flag(&self, id: i64, flag: &str) -> Result<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE events SET flag = ?2 WHERE id = ?1",
            params![id, flag],
        )?;
        Ok(())
    }

    /// Package installs the threat-intel worker has not looked at yet.
    /// Returns (id, ts, summary).
    pub fn unchecked_package_events(&self, limit: i64) -> Result<Vec<(i64, String, String)>> {
        let conn = self.read_conn();
        let mut stmt = conn.prepare(
            "SELECT id, ts, COALESCE(summary, '') FROM events
             WHERE kind = 'package_install' AND intel_checked = 0
             ORDER BY id DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn mark_intel_checked(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE events SET intel_checked = 1 WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    /// Event counts per (agent, source): the raw material for the capture
    /// status panel ("are hooks live? is tailing working?").
    pub fn capture_counts(&self) -> Result<Vec<CaptureCount>> {
        let conn = self.read_conn();
        let mut stmt = conn.prepare(
            "SELECT agent, source, COUNT(*), MAX(ts) FROM events GROUP BY agent, source",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(CaptureCount {
                agent: row.get(0)?,
                source: row.get(1)?,
                count: row.get(2)?,
                last_at: row.get(3)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    /// Delete events older than the cutoff (an RFC 3339 UTC string; all
    /// stored timestamps share the format, so string comparison is correct).
    /// Returns how many rows were removed.
    pub fn purge_events_before(&self, cutoff: &str) -> Result<usize> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let deleted = conn.execute("DELETE FROM events WHERE ts < ?1", params![cutoff])?;
        Ok(deleted)
    }

    pub fn setting(&self, key: &str) -> Result<Option<String>> {
        let conn = self.read_conn();
        let value = conn
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .ok();
        Ok(value)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// See [ChangeToken]. Costs one MAX over the primary key plus a count of
    /// flagged rows (a few hundred, via the partial index), so the UI can
    /// poll it every few seconds instead of running the full stats scan.
    pub fn change_token(&self) -> Result<ChangeToken> {
        let conn = self.read_conn();
        let token = conn.query_row(
            "SELECT COALESCE(MAX(id), 0),
                    (SELECT COUNT(*) FROM events WHERE flag IS NOT NULL AND ack = 0),
                    (SELECT COUNT(*) FROM events WHERE flag IS NOT NULL AND ack = 1)
             FROM events",
            [],
            |row| {
                Ok(ChangeToken {
                    max_id: row.get(0)?,
                    open_flags: row.get(1)?,
                    acked_flags: row.get(2)?,
                })
            },
        )?;
        Ok(token)
    }

    pub fn stats(&self) -> Result<Stats> {
        let today_start = format!("{}T00:00:00Z", today_utc());
        let conn = self.read_conn();
        let mut stats = conn.query_row(
            "SELECT COUNT(DISTINCT session_id), COUNT(*),
                    SUM(CASE WHEN kind = 'tool_call' AND tool_name IN ('Bash', 'shell') THEN 1 ELSE 0 END),
                    SUM(CASE WHEN kind = 'package_install' THEN 1 ELSE 0 END),
                    SUM(CASE WHEN flag IS NOT NULL AND ack = 0 THEN 1 ELSE 0 END),
                    SUM(CASE WHEN flag IS NOT NULL AND ack = 1 THEN 1 ELSE 0 END),
                    MAX(ts)
             FROM events",
            [],
            |row| {
                Ok(Stats {
                    session_count: row.get(0)?,
                    event_count: row.get(1)?,
                    command_count: row.get::<_, Option<i64>>(2)?.unwrap_or(0),
                    package_count: row.get::<_, Option<i64>>(3)?.unwrap_or(0),
                    flagged_count: row.get::<_, Option<i64>>(4)?.unwrap_or(0),
                    acked_count: row.get::<_, Option<i64>>(5)?.unwrap_or(0),
                    last_event_at: row.get(6)?,
                    sessions_today: 0,
                    commands_today: 0,
                    packages_today: 0,
                })
            },
        )?;
        let today = conn.query_row(
            "SELECT COUNT(DISTINCT session_id),
                    SUM(CASE WHEN kind = 'tool_call' AND tool_name IN ('Bash', 'shell') THEN 1 ELSE 0 END),
                    SUM(CASE WHEN kind = 'package_install' THEN 1 ELSE 0 END)
             FROM events WHERE ts >= ?1",
            params![today_start],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<i64>>(1)?.unwrap_or(0),
                    row.get::<_, Option<i64>>(2)?.unwrap_or(0),
                ))
            },
        )?;
        (
            stats.sessions_today,
            stats.commands_today,
            stats.packages_today,
        ) = today;
        Ok(stats)
    }

    /// Event volume per UTC day for the last `days` days, oldest first.
    pub fn events_per_day(&self, days: i64) -> Result<Vec<DayCount>> {
        let cutoff = (time::OffsetDateTime::now_utc() - time::Duration::days(days))
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default();
        let conn = self.read_conn();
        let mut stmt = conn.prepare(
            "SELECT substr(ts, 1, 10) AS day, COUNT(*),
                    SUM(CASE WHEN flag IS NOT NULL THEN 1 ELSE 0 END)
             FROM events WHERE ts >= ?1
             GROUP BY day ORDER BY day ASC",
        )?;
        let rows = stmt.query_map(params![cutoff], |row| {
            Ok(DayCount {
                day: row.get(0)?,
                events: row.get(1)?,
                flagged: row.get::<_, Option<i64>>(2)?.unwrap_or(0),
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }
}

fn today_utc() -> String {
    let now = time::OffsetDateTime::now_utc();
    format!(
        "{:04}-{:02}-{:02}",
        now.year(),
        u8::from(now.month()),
        now.day()
    )
}

/// Additive schema changes for databases created by older builds.
fn migrate(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare("PRAGMA table_info(events)")?;
    let columns: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<std::result::Result<_, _>>()?;
    if !columns.iter().any(|c| c == "flag") {
        conn.execute("ALTER TABLE events ADD COLUMN flag TEXT", [])?;
    }
    if !columns.iter().any(|c| c == "intel_checked") {
        conn.execute(
            "ALTER TABLE events ADD COLUMN intel_checked INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }
    if !columns.iter().any(|c| c == "ack") {
        conn.execute(
            "ALTER TABLE events ADD COLUMN ack INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }
    // This index references the flag column, so it must come after the
    // column exists on databases from older builds.
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_events_flagged ON events(ts) WHERE flag IS NOT NULL",
        [],
    )?;
    Ok(())
}

fn row_to_event_lite(row: &Row<'_>) -> rusqlite::Result<AgentEvent> {
    let kind: String = row.get(4)?;
    let source: String = row.get(5)?;
    Ok(AgentEvent {
        id: row.get(0)?,
        agent: row.get(1)?,
        session_id: row.get(2)?,
        ts: row.get(3)?,
        kind: EventKind::parse(&kind),
        source: EventSource::parse(&source),
        cwd: row.get(6)?,
        tool_name: row.get(7)?,
        summary: row.get(8)?,
        flag: row.get(9)?,
        payload: serde_json::Value::Null,
        dedupe_key: None,
    })
}

fn row_to_event(row: &Row<'_>) -> rusqlite::Result<AgentEvent> {
    let kind: String = row.get(4)?;
    let source: String = row.get(5)?;
    let payload: String = row.get(10)?;
    Ok(AgentEvent {
        id: row.get(0)?,
        agent: row.get(1)?,
        session_id: row.get(2)?,
        ts: row.get(3)?,
        kind: EventKind::parse(&kind),
        source: EventSource::parse(&source),
        cwd: row.get(6)?,
        tool_name: row.get(7)?,
        summary: row.get(8)?,
        flag: row.get(9)?,
        payload: serde_json::from_str(&payload).unwrap_or(serde_json::Value::Null),
        dedupe_key: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::now_iso;

    fn sample_event(session: &str, dedupe: Option<&str>) -> AgentEvent {
        AgentEvent {
            id: None,
            agent: "claude-code".into(),
            session_id: session.into(),
            ts: now_iso(),
            kind: EventKind::ToolCall,
            source: EventSource::Hook,
            cwd: Some("/tmp/project".into()),
            tool_name: Some("Bash".into()),
            summary: Some("npm install leftpad".into()),
            flag: None,
            payload: serde_json::json!({"tool_input": {"command": "npm install leftpad"}}),
            dedupe_key: dedupe.map(String::from),
        }
    }

    #[test]
    fn insert_and_read_back() {
        let store = Store::open_in_memory().unwrap();
        assert!(store.insert(&sample_event("s1", None)).unwrap());

        let sessions = store.sessions().unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "s1");
        assert_eq!(sessions[0].command_count, 1);

        let events = store.events_for_session("s1", 100).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].summary.as_deref(), Some("npm install leftpad"));
    }

    #[test]
    fn search_matches_all_terms_across_fields() {
        let store = Store::open_in_memory().unwrap();
        let mut a = sample_event("s1", None);
        a.summary = Some("npm install express".into());
        a.cwd = Some("/work/api".into());
        store.insert(&a).unwrap();
        let mut b = sample_event("s2", None);
        b.summary = Some("cargo build".into());
        store.insert(&b).unwrap();

        let hits = store.search_events("install api", 50).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].summary.as_deref(), Some("npm install express"));
        assert!(store.search_events("nomatchxyz", 50).unwrap().is_empty());
        assert!(store.search_events("", 50).unwrap().is_empty());
    }

    #[test]
    fn dedupe_key_prevents_double_insert() {
        let store = Store::open_in_memory().unwrap();
        assert!(store.insert(&sample_event("s1", Some("k1"))).unwrap());
        assert!(!store.insert(&sample_event("s1", Some("k1"))).unwrap());
        assert_eq!(store.stats().unwrap().event_count, 1);
    }
}
