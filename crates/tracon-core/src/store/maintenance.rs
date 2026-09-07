//! Deleting and shrinking: retention purges, wiping sessions, and giving
//! freed pages back to the filesystem.

use std::path::Path;

use anyhow::Result;
use rusqlite::{params, Connection};

use super::{rollup, Store};
use crate::clock;

/// Rows deleted per purge transaction. Small enough that a UI read never
/// waits long behind the writer, large enough that a 90 day backlog
/// clears in a handful of commits.
const PURGE_BATCH: usize = 5_000;

impl Store {
    /// Delete events older than the cutoff (a stored-form UTC string; all
    /// stored timestamps share the format, so string comparison is correct).
    /// Returns how many rows were removed.
    pub fn purge_events_before(&self, cutoff: &str) -> Result<usize> {
        let mut total = 0;
        loop {
            let deleted = self.purge_events_before_batch(cutoff, PURGE_BATCH)?;
            total += deleted;
            if deleted < PURGE_BATCH {
                break;
            }
        }
        if total > 0 {
            self.rebuild_rollup()?;
        }
        Ok(total)
    }

    /// One bounded slice of purge_events_before. Does NOT refresh the
    /// session rollup; callers looping over it must call rebuild_rollup
    /// once they are done (purge_events_before does).
    pub fn purge_events_before_batch(&self, cutoff: &str, limit: usize) -> Result<usize> {
        let cutoff = clock::canonical_cutoff(cutoff);
        let conn = self.writer();
        let deleted = conn.execute(
            "DELETE FROM events WHERE id IN
               (SELECT id FROM events WHERE ts < ?1 LIMIT ?2)",
            params![cutoff, limit as i64],
        )?;
        Ok(deleted)
    }

    /// Wipe every event and session. Settings and tail offsets survive so
    /// capture resumes where it left off without re-importing history.
    pub fn purge_all(&self) -> Result<()> {
        let mut conn = self.writer();
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM events", [])?;
        tx.execute("DELETE FROM session_rollup", [])?;
        tx.commit()?;
        Ok(())
    }

    /// Delete one session's events and its rollup row. Returns the number
    /// of events removed.
    pub fn purge_session(&self, session_id: &str) -> Result<usize> {
        let mut conn = self.writer();
        let tx = conn.transaction()?;
        let deleted = tx.execute(
            "DELETE FROM events WHERE session_id = ?1",
            params![session_id],
        )?;
        tx.execute(
            "DELETE FROM session_rollup WHERE session_id = ?1",
            params![session_id],
        )?;
        tx.commit()?;
        Ok(deleted)
    }

    /// Hand freed pages back to the OS: fold the WAL into the main file and
    /// run the incremental vacuum (a no-op on databases created before
    /// auto_vacuum was enabled, see Store::from_conn). Also forgets tail
    /// offsets for log files that no longer exist.
    pub fn compact(&self) -> Result<()> {
        self.purge_missing_tail_offsets()?;
        let conn = self.writer();
        run_pragma(&conn, "PRAGMA wal_checkpoint(TRUNCATE)")?;
        run_pragma(&conn, "PRAGMA incremental_vacuum")?;
        Ok(())
    }

    /// Drop tail offsets whose file is gone (rotated or deleted transcripts).
    /// Returns how many rows were removed.
    pub fn purge_missing_tail_offsets(&self) -> Result<usize> {
        let missing: Vec<String> = self
            .tail_offset_paths()?
            .into_iter()
            .filter(|path| !Path::new(path).exists())
            .collect();
        if missing.is_empty() {
            return Ok(0);
        }
        let mut conn = self.writer();
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare("DELETE FROM tail_offsets WHERE path = ?1")?;
            for path in &missing {
                stmt.execute(params![path])?;
            }
        }
        tx.commit()?;
        Ok(missing.len())
    }

    /// Rebuild the session rollup from the events table. Runs on its own
    /// after bulk deletes; exposed for callers that suspect drift.
    pub fn rebuild_rollup(&self) -> Result<()> {
        let mut conn = self.writer();
        let tx = conn.transaction()?;
        rollup::rebuild(&tx)?;
        tx.commit()?;
        Ok(())
    }
}

/// Some pragmas return rows (wal_checkpoint reports what it did); draining
/// them is the portable way to run one regardless of its return shape.
fn run_pragma(conn: &Connection, sql: &str) -> Result<()> {
    let mut stmt = conn.prepare(sql)?;
    let mut rows = stmt.query([])?;
    while rows.next()?.is_some() {}
    Ok(())
}
