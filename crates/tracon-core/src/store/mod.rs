mod flags;
mod maintenance;
mod packages;
mod rollup;
mod sessions;
mod settings;
mod stats;
mod types;

#[cfg(test)]
mod tests;

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard};

use anyhow::Result;
use rusqlite::{params, Connection, OpenFlags, Row, Transaction};

use crate::clock;
use crate::event::{AgentEvent, EventKind, EventSource};

pub use types::{
    CaptureCount, ChangeToken, DayCount, LiveSession, SessionSummary, Stats, LIVE_WINDOW_MINUTES,
};

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

/// SQL fragments shared by the per-session queries and the rollup rebuild,
/// so "what counts as a command" is defined once.
pub(super) const COMMAND_COUNT_CASE: &str =
    "CASE WHEN kind = 'tool_call' AND tool_name IN ('Bash', 'shell') THEN 1 ELSE 0 END";

/// The session's most recent non-null cwd. MAX(cwd) would pick the
/// lexicographically greatest path instead of the latest one.
pub(super) const LATEST_CWD_SUBQUERY: &str = "(SELECT e1.cwd FROM events e1
     WHERE e1.session_id = events.session_id AND e1.cwd IS NOT NULL
     ORDER BY e1.ts DESC, e1.id DESC LIMIT 1)";

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
        // auto_vacuum only takes effect on a database created after it is
        // set; existing files keep whatever mode they were created with.
        // On a fresh file it lets compact() return freed pages to the OS.
        conn.pragma_update(None, "auto_vacuum", "INCREMENTAL")?;
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

    /// The single write connection. A panic while holding the lock poisons
    /// the mutex, but the connection itself is still fine (SQLite rolled
    /// back whatever was in flight), so recovering the guard keeps the app
    /// alive instead of turning one crashed worker into a dead store.
    fn writer(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn read_conn(&self) -> MutexGuard<'_, Connection> {
        if self.readers.is_empty() {
            return self.writer();
        }
        let i = self.next_reader.fetch_add(1, Ordering::Relaxed) % self.readers.len();
        self.readers[i].lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Insert an event. Returns false when the dedupe key says we already have it.
    pub fn insert(&self, event: &AgentEvent) -> Result<bool> {
        let mut conn = self.writer();
        let tx = conn.transaction()?;
        let inserted = insert_in_tx(&tx, event)?;
        tx.commit()?;
        Ok(inserted)
    }

    /// Insert many events in one transaction (one fsync instead of one per
    /// row, which is what makes tailing a large transcript fast). Returns
    /// how many rows were new; dedupe hits are skipped silently.
    pub fn insert_batch(&self, events: &[AgentEvent]) -> Result<usize> {
        let mut conn = self.writer();
        let tx = conn.transaction()?;
        let mut inserted = 0;
        for event in events {
            if insert_in_tx(&tx, event)? {
                inserted += 1;
            }
        }
        tx.commit()?;
        Ok(inserted)
    }
}

fn insert_in_tx(tx: &Transaction<'_>, event: &AgentEvent) -> Result<bool> {
    let payload = serde_json::to_string(&event.payload)?;
    let ts = clock::normalize_ts(&event.ts, time::OffsetDateTime::now_utc());
    let inserted = tx.execute(
        "INSERT OR IGNORE INTO events
           (agent, session_id, ts, kind, source, cwd, tool_name, summary, flag, payload, dedupe_key)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            event.agent,
            event.session_id,
            ts,
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
    if inserted == 0 {
        return Ok(false);
    }
    rollup::apply_event(tx, event, &ts)?;
    Ok(true)
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
    // Mirrors idx_events_prompt: the live board's last-action subquery needs
    // the same one-probe lookup for tool calls.
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_events_tool_call ON events(session_id, ts) WHERE kind = 'tool_call'",
        [],
    )?;
    conn.execute_batch(rollup::SCHEMA)?;
    let rollup_is_empty: bool = conn.query_row(
        "SELECT NOT EXISTS (SELECT 1 FROM session_rollup)",
        [],
        |row| row.get(0),
    )?;
    if rollup_is_empty {
        rollup::rebuild(conn)?;
    }
    Ok(())
}

pub(super) fn row_to_event_lite(row: &Row<'_>) -> rusqlite::Result<AgentEvent> {
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

pub(super) fn row_to_event(row: &Row<'_>) -> rusqlite::Result<AgentEvent> {
    let mut event = row_to_event_lite(row)?;
    let payload: String = row.get(10)?;
    event.payload = serde_json::from_str(&payload).unwrap_or(serde_json::Value::Null);
    Ok(event)
}

/// Collect a rusqlite row iterator into a Vec, converting the error type.
pub(super) fn collect_rows<T>(rows: impl Iterator<Item = rusqlite::Result<T>>) -> Result<Vec<T>> {
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}
