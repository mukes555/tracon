use anyhow::Result;
use rusqlite::params;

use super::Store;

impl Store {
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
        let conn = self.writer();
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// True while the user has paused capture from the tray.
    pub fn capture_paused(&self) -> bool {
        self.setting("capture_paused")
            .ok()
            .flatten()
            .is_some_and(|v| v == "true")
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
        let conn = self.writer();
        conn.execute(
            "INSERT INTO tail_offsets (path, offset) VALUES (?1, ?2)
             ON CONFLICT(path) DO UPDATE SET offset = excluded.offset",
            params![path, offset as i64],
        )?;
        Ok(())
    }

    /// Every tailed path we hold an offset for.
    pub fn tail_offset_paths(&self) -> Result<Vec<String>> {
        let conn = self.read_conn();
        let mut stmt = conn.prepare("SELECT path FROM tail_offsets ORDER BY path")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        super::collect_rows(rows)
    }
}
