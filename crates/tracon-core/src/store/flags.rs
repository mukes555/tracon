use anyhow::Result;
use rusqlite::params;

use super::{collect_rows, rollup, row_to_event_lite, Store};
use crate::event::AgentEvent;

impl Store {
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
        collect_rows(rows)
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
        collect_rows(rows)
    }

    /// Mark flagged events reviewed (or reopen them) in one transaction, so
    /// "acknowledge all" costs one commit instead of one per row.
    pub fn set_ack(&self, ids: &[i64], acked: bool) -> Result<()> {
        let mut conn = self.writer();
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare("UPDATE events SET ack = ?2 WHERE id = ?1")?;
            for id in ids {
                stmt.execute(params![id, acked as i64])?;
            }
        }
        rollup::refresh_flag_counts(&tx, ids)?;
        tx.commit()?;
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
        collect_rows(rows)
    }

    pub fn set_flag(&self, id: i64, flag: &str) -> Result<()> {
        let mut conn = self.writer();
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE events SET flag = ?2 WHERE id = ?1",
            params![id, flag],
        )?;
        rollup::refresh_flag_counts(&tx, &[id])?;
        tx.commit()?;
        Ok(())
    }
}
