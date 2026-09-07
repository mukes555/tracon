use anyhow::Result;
use rusqlite::params;

use super::{collect_rows, row_to_event_lite, Store};
use crate::event::AgentEvent;

impl Store {
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
        collect_rows(rows)
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
        collect_rows(rows)
    }

    pub fn mark_intel_checked(&self, id: i64) -> Result<()> {
        let conn = self.writer();
        conn.execute(
            "UPDATE events SET intel_checked = 1 WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }
}
