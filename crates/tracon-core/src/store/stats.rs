use anyhow::Result;
use rusqlite::params;
use time::{OffsetDateTime, UtcOffset};

use super::{
    collect_rows, CaptureCount, ChangeToken, DayCount, Stats, Store, COMMAND_COUNT_CASE,
    LIVE_WINDOW_MINUTES,
};
use crate::clock;

impl Store {
    /// Headline numbers for the dashboard. "Today" is the user's local day,
    /// not the UTC one, so an evening session does not vanish at 5pm on the
    /// US west coast.
    pub fn stats(&self) -> Result<Stats> {
        let day_start = clock::local_day_start(OffsetDateTime::now_utc(), clock::local_offset());
        self.stats_since_day_start(&clock::format_ts(day_start))
    }

    /// stats() with an explicit day boundary (a stored-form timestamp), so
    /// the local-midnight logic can be tested without touching the clock.
    pub(super) fn stats_since_day_start(&self, today_start: &str) -> Result<Stats> {
        let conn = self.read_conn();
        // Totals come from the rollup (a few hundred rows) instead of a scan
        // of every event; the two counts the rollup does not carry use the
        // partial flag index and the kind index, both tiny ranges.
        let mut stats = conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(event_count), 0), COALESCE(SUM(command_count), 0),
                    (SELECT COUNT(*) FROM events WHERE kind = 'package_install'),
                    COALESCE(SUM(flagged_open_count), 0),
                    (SELECT COUNT(*) FROM events WHERE flag IS NOT NULL AND ack = 1),
                    MAX(last_at),
                    (SELECT COUNT(*) FROM session_rollup WHERE last_at >= ?1)
             FROM session_rollup",
            params![today_start],
            |row| {
                Ok(Stats {
                    session_count: row.get(0)?,
                    event_count: row.get(1)?,
                    command_count: row.get(2)?,
                    package_count: row.get(3)?,
                    flagged_count: row.get(4)?,
                    acked_count: row.get(5)?,
                    last_event_at: row.get(6)?,
                    sessions_today: row.get(7)?,
                    commands_today: 0,
                    packages_today: 0,
                })
            },
        )?;
        let today_sql = format!(
            "SELECT SUM({COMMAND_COUNT_CASE}),
                    SUM(CASE WHEN kind = 'package_install' THEN 1 ELSE 0 END)
             FROM events INDEXED BY idx_events_ts WHERE ts >= ?1"
        );
        let (commands_today, packages_today) =
            conn.query_row(&today_sql, params![today_start], |row| {
                Ok((
                    row.get::<_, Option<i64>>(0)?.unwrap_or(0),
                    row.get::<_, Option<i64>>(1)?.unwrap_or(0),
                ))
            })?;
        stats.commands_today = commands_today;
        stats.packages_today = packages_today;
        Ok(stats)
    }

    /// See [ChangeToken]. Costs one MAX over the primary key plus a count of
    /// flagged rows (a few hundred, via the partial index), so the UI can
    /// poll it every few seconds instead of running the full stats scan.
    pub fn change_token(&self) -> Result<ChangeToken> {
        let conn = self.read_conn();
        let cutoff = clock::minutes_ago(LIVE_WINDOW_MINUTES);
        let token = conn.query_row(
            "SELECT COALESCE(MAX(id), 0),
                    (SELECT COUNT(*) FROM events WHERE flag IS NOT NULL AND ack = 0),
                    (SELECT COUNT(*) FROM events WHERE flag IS NOT NULL AND ack = 1),
                    (SELECT COUNT(DISTINCT session_id) FROM events INDEXED BY idx_events_ts
                     WHERE ts >= ?1 AND agent != 'system')
             FROM events",
            params![cutoff],
            |row| {
                Ok(ChangeToken {
                    max_id: row.get(0)?,
                    open_flags: row.get(1)?,
                    acked_flags: row.get(2)?,
                    live_sessions: row.get(3)?,
                })
            },
        )?;
        Ok(token)
    }

    /// Event volume per local day for the last `days` days, oldest first.
    pub fn events_per_day(&self, days: i64) -> Result<Vec<DayCount>> {
        self.events_per_day_at_offset(days, clock::local_offset())
    }

    /// events_per_day with an explicit UTC offset for the day buckets.
    /// The shift happens inside SQLite (date() with a seconds modifier) so
    /// the window's rows never have to cross into Rust just to be counted.
    pub(super) fn events_per_day_at_offset(
        &self,
        days: i64,
        offset: UtcOffset,
    ) -> Result<Vec<DayCount>> {
        let cutoff = clock::days_ago(days);
        let shift = format!("{} seconds", offset.whole_seconds());
        let conn = self.read_conn();
        // Rows whose ts predates timestamp normalization may not parse in
        // SQLite; they fall back to their raw calendar date.
        let mut stmt = conn.prepare(
            "SELECT COALESCE(date(ts, ?2), substr(ts, 1, 10)) AS day, COUNT(*),
                    SUM(CASE WHEN flag IS NOT NULL THEN 1 ELSE 0 END)
             FROM events WHERE ts >= ?1
             GROUP BY day ORDER BY day ASC",
        )?;
        let rows = stmt.query_map(params![cutoff, shift], |row| {
            Ok(DayCount {
                day: row.get(0)?,
                events: row.get(1)?,
                flagged: row.get::<_, Option<i64>>(2)?.unwrap_or(0),
            })
        })?;
        collect_rows(rows)
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
        collect_rows(rows)
    }

    /// Agents (excluding the system pseudo-agent) with any event at or after
    /// the cutoff: "who was active on this machine just now".
    pub fn agents_active_since(&self, cutoff_ts: &str) -> Result<Vec<String>> {
        let cutoff_ts = clock::canonical_cutoff(cutoff_ts);
        let conn = self.read_conn();
        let mut stmt = conn.prepare(
            "SELECT DISTINCT agent FROM events WHERE ts >= ?1 AND agent != 'system' ORDER BY agent",
        )?;
        let rows = stmt.query_map(params![cutoff_ts], |row| row.get(0))?;
        collect_rows(rows)
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
}
