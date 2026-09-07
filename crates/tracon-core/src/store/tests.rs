use super::*;
use crate::event::{EventKind, EventSource};
use crate::now_iso;
use rusqlite::Connection;
use time::macros::datetime;
use time::UtcOffset;

/// A Bash tool call in `session` at `ts`; tests tweak the fields they care about.
fn event_at(session: &str, ts: &str) -> AgentEvent {
    AgentEvent {
        id: None,
        agent: "claude-code".into(),
        session_id: session.into(),
        ts: ts.into(),
        kind: EventKind::ToolCall,
        source: EventSource::Hook,
        cwd: Some("/tmp/project".into()),
        tool_name: Some("Bash".into()),
        summary: Some("npm install leftpad".into()),
        flag: None,
        payload: serde_json::json!({"tool_input": {"command": "npm install leftpad"}}),
        dedupe_key: None,
    }
}

fn sample_event(session: &str, dedupe: Option<&str>) -> AgentEvent {
    let mut event = event_at(session, &now_iso());
    event.dedupe_key = dedupe.map(String::from);
    event
}

fn task_call(session: &str, ts: &str, subagent: &str) -> AgentEvent {
    let mut event = event_at(session, ts);
    event.tool_name = Some("Task".into());
    event.summary = Some("Task".into());
    event.payload = serde_json::json!({"tool_input": {"subagent_type": subagent}});
    event
}

fn flagged(session: &str, ts: &str) -> AgentEvent {
    let mut event = event_at(session, ts);
    event.flag = Some("destructive delete".into());
    event
}

fn ids_of(events: &[AgentEvent]) -> Vec<i64> {
    events.iter().filter_map(|e| e.id).collect()
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
fn search_escapes_like_wildcards() {
    let store = Store::open_in_memory().unwrap();
    let mut literal = sample_event("s1", None);
    literal.summary = Some("echo 100%_done".into());
    store.insert(&literal).unwrap();
    let mut decoy = sample_event("s2", None);
    decoy.summary = Some("echo 100 done".into());
    store.insert(&decoy).unwrap();

    let percent_hits = store.search_events("100%", 50).unwrap();
    assert_eq!(percent_hits.len(), 1);
    assert_eq!(percent_hits[0].session_id, "s1");
    let underscore_hits = store.search_events("%_done", 50).unwrap();
    assert_eq!(underscore_hits.len(), 1);
    // Without escaping, "100_done" would match "100 done" via the _ wildcard.
    assert!(store.search_events("100_done", 50).unwrap().is_empty());
}

#[test]
fn dedupe_key_prevents_double_insert() {
    let store = Store::open_in_memory().unwrap();
    assert!(store.insert(&sample_event("s1", Some("k1"))).unwrap());
    assert!(!store.insert(&sample_event("s1", Some("k1"))).unwrap());
    assert_eq!(store.stats().unwrap().event_count, 1);
}

#[test]
fn rows_without_dedupe_key_never_collide() {
    let store = Store::open_in_memory().unwrap();
    for _ in 0..3 {
        assert!(store.insert(&sample_event("s1", None)).unwrap());
    }
    assert_eq!(store.stats().unwrap().event_count, 3);
}

#[test]
fn insert_batch_counts_only_new_rows() {
    let store = Store::open_in_memory().unwrap();
    let events = vec![
        sample_event("s1", Some("a")),
        sample_event("s1", Some("a")),
        sample_event("s2", Some("b")),
        sample_event("s2", None),
    ];
    assert_eq!(store.insert_batch(&events).unwrap(), 3);
    assert_eq!(store.sessions().unwrap().len(), 2);
    assert_eq!(store.stats().unwrap().event_count, 3);
}

#[test]
fn timestamps_are_normalized_on_insert() {
    let store = Store::open_in_memory().unwrap();
    store
        .insert(&event_at("s1", "2026-01-05T11:30:00+02:00"))
        .unwrap();
    store.insert(&event_at("s1", "garbage")).unwrap();
    store
        .insert(&event_at("s1", "2999-01-01T00:00:00Z"))
        .unwrap();

    let events = store.events_for_session("s1", 10).unwrap();
    assert_eq!(events[0].ts, "2026-01-05T09:30:00.000Z");
    let now = now_iso();
    for event in &events[1..] {
        assert!(event.ts.ends_with('Z'));
        assert!(event.ts.starts_with(&now[..13]), "{} vs {now}", event.ts);
    }
}

#[test]
fn live_sessions_respects_window_system_agent_and_latest_cwd() {
    let store = Store::open_in_memory().unwrap();
    let mut old = event_at("old", "2026-01-01T00:00:00Z");
    old.cwd = Some("/old".into());
    store.insert(&old).unwrap();

    let mut system = event_at("sys", "2026-01-02T00:00:10Z");
    system.agent = "system".into();
    store.insert(&system).unwrap();

    let mut first = event_at("live", "2026-01-02T00:00:05Z");
    first.cwd = Some("/first".into());
    store.insert(&first).unwrap();
    let mut second = event_at("live", "2026-01-02T00:00:07Z");
    second.cwd = Some("/second".into());
    store.insert(&second).unwrap();
    let mut third = event_at("live", "2026-01-02T00:00:09Z");
    third.cwd = None;
    store.insert(&third).unwrap();

    let live = store.live_sessions("2026-01-02T00:00:00Z", 10).unwrap();
    assert_eq!(live.len(), 1);
    assert_eq!(live[0].session_id, "live");
    assert_eq!(live[0].cwd.as_deref(), Some("/second"));
    assert_eq!(live[0].event_count, 3);
}

#[test]
fn subagents_count_calls_once_and_labels_dedupe_and_cap() {
    let store = Store::open_in_memory().unwrap();
    let base = "2026-01-02T00:00:0";
    for (i, name) in ["explorer", "explorer", "planner", "reviewer", "tester"]
        .iter()
        .enumerate()
    {
        store
            .insert(&task_call("s", &format!("{base}{i}Z"), name))
            .unwrap();
    }
    let mut result = task_call("s", "2026-01-02T00:00:09Z", "tester");
    result.kind = EventKind::ToolResult;
    store.insert(&result).unwrap();

    let live = store.live_sessions("2026-01-02T00:00:00Z", 10).unwrap();
    assert_eq!(live[0].subagent_count, 5);
    assert_eq!(live[0].subagents, vec!["tester", "reviewer", "planner"]);
}

#[test]
fn set_ack_splits_buckets_and_reopens() {
    let store = Store::open_in_memory().unwrap();
    for i in 0..3 {
        store
            .insert(&flagged("s", &format!("2026-01-02T00:00:0{i}Z")))
            .unwrap();
    }
    let open = store.flagged_events(10, false).unwrap();
    assert_eq!(open.len(), 3);
    let ids = ids_of(&open);

    store.set_ack(&ids[..2], true).unwrap();
    assert_eq!(store.flagged_events(10, false).unwrap().len(), 1);
    assert_eq!(store.flagged_events(10, true).unwrap().len(), 2);
    assert_eq!(store.sessions().unwrap()[0].flagged_count, 1);

    store.set_ack(&ids[..1], false).unwrap();
    assert_eq!(store.flagged_events(10, false).unwrap().len(), 2);
    assert_eq!(store.sessions().unwrap()[0].flagged_count, 2);
    let stats = store.stats().unwrap();
    assert_eq!((stats.flagged_count, stats.acked_count), (2, 1));
}

#[test]
fn change_token_moves_on_insert_and_ack() {
    let store = Store::open_in_memory().unwrap();
    let empty = store.change_token().unwrap();
    store.insert(&flagged("s", &now_iso())).unwrap();
    let after_insert = store.change_token().unwrap();
    assert_ne!(empty, after_insert);
    assert_eq!(after_insert.live_sessions, 1);

    let ids = ids_of(&store.flagged_events(10, false).unwrap());
    store.set_ack(&ids, true).unwrap();
    let after_ack = store.change_token().unwrap();
    assert_ne!(after_insert, after_ack);
    assert_eq!((after_ack.open_flags, after_ack.acked_flags), (0, 1));
}

#[test]
fn set_flag_backfill_updates_session_count() {
    let store = Store::open_in_memory().unwrap();
    store
        .insert(&event_at("s", "2026-01-02T00:00:00Z"))
        .unwrap();
    let pending = store.unassessed_bash_events(10).unwrap();
    assert_eq!(pending.len(), 1);
    store.set_flag(pending[0].0, "destructive delete").unwrap();
    assert_eq!(store.sessions().unwrap()[0].flagged_count, 1);
    assert!(store.unassessed_bash_events(10).unwrap().is_empty());
}

#[test]
fn stats_today_uses_the_given_local_day_start() {
    let store = Store::open_in_memory().unwrap();
    // Day boundary for UTC+2 on 2026-01-05 is 2026-01-04T22:00:00Z.
    let plus_two = UtcOffset::from_hms(2, 0, 0).unwrap();
    let now = datetime!(2026-01-05 10:00:00 UTC);
    let day_start = clock::format_ts(clock::local_day_start(now, plus_two));

    let mut yesterday = event_at("yesterday", "2026-01-04T23:59:59+02:00");
    yesterday.kind = EventKind::PackageInstall;
    store.insert(&yesterday).unwrap();
    store
        .insert(&event_at("today", "2026-01-05T00:00:01+02:00"))
        .unwrap();

    let stats = store.stats_since_day_start(&day_start).unwrap();
    assert_eq!(stats.session_count, 2);
    assert_eq!(stats.sessions_today, 1);
    assert_eq!(stats.commands_today, 1);
    assert_eq!(stats.packages_today, 0);
    assert_eq!(stats.package_count, 1);
    assert_eq!(
        stats.last_event_at.as_deref(),
        Some("2026-01-04T22:00:01.000Z")
    );
}

#[test]
fn events_per_day_buckets_by_local_day() {
    let store = Store::open_in_memory().unwrap();
    let recent = time::OffsetDateTime::now_utc() - time::Duration::days(1);
    let late_evening_utc = recent.replace_time(time::Time::from_hms(23, 30, 0).unwrap());
    store
        .insert(&event_at("s", &clock::format_ts(late_evening_utc)))
        .unwrap();

    let utc_days = store.events_per_day_at_offset(7, UtcOffset::UTC).unwrap();
    let plus_two_days = store
        .events_per_day_at_offset(7, UtcOffset::from_hms(2, 0, 0).unwrap())
        .unwrap();
    assert_eq!(utc_days.len(), 1);
    assert_eq!(plus_two_days.len(), 1);
    assert_eq!(utc_days[0].day, clock::format_ts(late_evening_utc)[..10]);
    let next_day = late_evening_utc + time::Duration::hours(2);
    assert_eq!(plus_two_days[0].day, clock::format_ts(next_day)[..10]);
    assert_eq!(plus_two_days[0].events, 1);
}

/// The old sessions() query, recomputed from events, grouped by session.
fn sessions_from_events(store: &Store) -> Vec<SessionSummary> {
    let conn = store.read_conn();
    let sql = format!(
        "SELECT session_id,
                (SELECT e0.agent FROM events e0 WHERE e0.session_id = events.session_id
                 ORDER BY e0.ts ASC, e0.id ASC LIMIT 1),
                {LATEST_CWD_SUBQUERY},
                MIN(ts), MAX(ts), COUNT(*),
                SUM({COMMAND_COUNT_CASE}),
                SUM(CASE WHEN flag IS NOT NULL AND ack = 0 THEN 1 ELSE 0 END),
                SUM(CASE WHEN kind = 'tool_call' AND source = 'hook' THEN 1 ELSE 0 END),
                SUM(CASE WHEN kind = 'tool_call' AND source = 'log_tail' THEN 1 ELSE 0 END),
                (SELECT e2.summary FROM events e2
                 WHERE e2.session_id = events.session_id AND e2.kind = 'prompt'
                 ORDER BY e2.ts ASC, e2.id ASC LIMIT 1)
         FROM events GROUP BY session_id ORDER BY MAX(ts) DESC"
    );
    let mut stmt = conn.prepare(&sql).unwrap();
    let rows = stmt
        .query_map([], |row| {
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
        })
        .unwrap();
    rows.map(Result::unwrap).collect()
}

fn assert_same_sessions(rollup: &[SessionSummary], recomputed: &[SessionSummary]) {
    assert_eq!(rollup.len(), recomputed.len());
    for (a, b) in rollup.iter().zip(recomputed) {
        assert_eq!(format!("{a:?}"), format!("{b:?}"));
    }
}

#[test]
fn rollup_matches_recomputation_after_mixed_writes() {
    let store = Store::open_in_memory().unwrap();
    let mut prompt = event_at("s1", "2026-01-02T00:00:02Z");
    prompt.kind = EventKind::Prompt;
    prompt.tool_name = None;
    prompt.summary = Some("second prompt".into());
    store.insert(&prompt).unwrap();
    // An earlier prompt arriving later (transcript backfill) must win.
    let mut earlier_prompt = prompt.clone();
    earlier_prompt.ts = "2026-01-02T00:00:01Z".into();
    earlier_prompt.summary = Some("first prompt".into());
    store.insert(&earlier_prompt).unwrap();

    let mut tail_call = event_at("s1", "2026-01-02T00:00:03Z");
    tail_call.source = EventSource::LogTail;
    tail_call.cwd = Some("/late".into());
    store.insert(&tail_call).unwrap();
    let mut older_cwd = event_at("s1", "2026-01-01T00:00:00Z");
    older_cwd.cwd = Some("/early".into());
    store.insert(&older_cwd).unwrap();
    let mut no_cwd = event_at("s1", "2026-01-02T00:00:04Z");
    no_cwd.cwd = None;
    store.insert(&no_cwd).unwrap();

    let mut s1_flag = flagged("s1", "2026-01-02T00:00:05Z");
    s1_flag.cwd = None;
    store.insert(&s1_flag).unwrap();
    store
        .insert(&flagged("s2", "2026-01-02T00:00:06Z"))
        .unwrap();
    let mut shell = event_at("s2", "2026-01-02T00:00:07Z");
    shell.tool_name = Some("shell".into());
    shell.agent = "codex".into();
    store.insert(&shell).unwrap();
    let mut read_call = event_at("s2", "2026-01-02T00:00:08Z");
    read_call.tool_name = Some("Read".into());
    store.insert(&read_call).unwrap();

    assert_same_sessions(&store.sessions().unwrap(), &sessions_from_events(&store));

    let flagged_ids = ids_of(&store.flagged_events(10, false).unwrap());
    store.set_ack(&flagged_ids, true).unwrap();
    store.set_ack(&flagged_ids[..1], false).unwrap();
    let pending = store.unassessed_bash_events(10).unwrap();
    store.set_flag(pending[0].0, "force push").unwrap();

    let rollup = store.sessions().unwrap();
    assert_same_sessions(&rollup, &sessions_from_events(&store));
    let s1 = rollup.iter().find(|s| s.session_id == "s1").unwrap();
    assert_eq!(s1.cwd.as_deref(), Some("/late"));
    assert_eq!(s1.first_prompt.as_deref(), Some("first prompt"));
    assert_eq!(s1.started_at, "2026-01-01T00:00:00.000Z");
    assert_eq!((s1.hook_tool_count, s1.tail_tool_count), (3, 1));

    store.rebuild_rollup().unwrap();
    assert_same_sessions(&store.sessions().unwrap(), &sessions_from_events(&store));
}

#[test]
fn purges_rebuild_rollup_and_report_counts() {
    let store = Store::open_in_memory().unwrap();
    for i in 0..7 {
        store
            .insert(&event_at("old", &format!("2026-01-01T00:00:0{i}Z")))
            .unwrap();
    }
    store
        .insert(&event_at("new", "2026-01-03T00:00:00Z"))
        .unwrap();
    store
        .insert(&event_at("gone", "2026-01-03T00:00:01Z"))
        .unwrap();

    assert_eq!(
        store
            .purge_events_before_batch("2026-01-02T00:00:00Z", 3)
            .unwrap(),
        3
    );
    assert_eq!(
        store.purge_events_before("2026-01-02T00:00:00Z").unwrap(),
        4
    );
    let sessions = store.sessions().unwrap();
    assert_eq!(sessions.len(), 2);
    assert!(sessions.iter().all(|s| s.session_id != "old"));

    assert_eq!(store.purge_session("gone").unwrap(), 1);
    assert_eq!(store.sessions().unwrap().len(), 1);
    assert_eq!(store.max_event_id().unwrap(), 8);

    store.purge_all().unwrap();
    assert!(store.sessions().unwrap().is_empty());
    assert_eq!(store.stats().unwrap().event_count, 0);
    assert_eq!(store.max_event_id().unwrap(), 0);
}

#[test]
fn compact_drops_offsets_for_missing_files() {
    let store = Store::open_in_memory().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let existing = dir.path().join("live.jsonl");
    std::fs::write(&existing, "x").unwrap();
    let existing_key = existing.to_string_lossy().to_string();
    let missing_key = dir.path().join("gone.jsonl").to_string_lossy().to_string();
    store.set_tail_offset(&existing_key, 10).unwrap();
    store.set_tail_offset(&missing_key, 20).unwrap();

    store.compact().unwrap();
    assert_eq!(
        store.tail_offset_paths().unwrap(),
        vec![existing_key.clone()]
    );
    assert_eq!(store.tail_offset(&existing_key).unwrap(), 10);
    assert_eq!(store.tail_offset(&missing_key).unwrap(), 0);
}

#[test]
fn migrate_upgrades_a_pre_flag_database() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("old.db");
    {
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE events (
               id INTEGER PRIMARY KEY, agent TEXT NOT NULL, session_id TEXT NOT NULL,
               ts TEXT NOT NULL, kind TEXT NOT NULL, source TEXT NOT NULL, cwd TEXT,
               tool_name TEXT, summary TEXT, payload TEXT NOT NULL, dedupe_key TEXT UNIQUE);
             INSERT INTO events (agent, session_id, ts, kind, source, cwd, tool_name, summary, payload)
             VALUES ('claude-code', 'legacy', '2026-01-01T00:00:00Z', 'tool_call', 'hook',
                     '/legacy', 'Bash', 'ls', '{}');",
        )
        .unwrap();
    }

    let store = Store::open(&path).unwrap();
    {
        let conn = store.read_conn();
        let mut stmt = conn.prepare("PRAGMA table_info(events)").unwrap();
        let columns: Vec<String> = stmt
            .query_map([], |row| row.get(1))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        for column in ["flag", "ack", "intel_checked"] {
            assert!(columns.iter().any(|c| c == column), "missing {column}");
        }
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'index'")
            .unwrap();
        let indexes: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        for index in [
            "idx_events_flagged",
            "idx_events_tool_call",
            "idx_events_ts",
            "idx_rollup_last_at",
        ] {
            assert!(indexes.iter().any(|i| i == index), "missing {index}");
        }
    }

    let sessions = store.sessions().unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].session_id, "legacy");
    assert_eq!(sessions[0].cwd.as_deref(), Some("/legacy"));
    assert!(store.flagged_events(10, false).unwrap().is_empty());
    assert!(store
        .insert(&event_at("legacy", "2026-01-01T00:01:00Z"))
        .unwrap());
    assert_eq!(store.sessions().unwrap()[0].event_count, 2);
}

#[test]
fn writer_survives_a_poisoned_mutex() {
    let store = std::sync::Arc::new(Store::open_in_memory().unwrap());
    let poisoner = store.clone();
    let _ = std::thread::spawn(move || {
        let _guard = poisoner.conn.lock().unwrap();
        panic!("poison the store lock on purpose");
    })
    .join();
    assert!(store.conn.is_poisoned());

    assert!(store.insert(&sample_event("s1", None)).unwrap());
    assert_eq!(store.sessions().unwrap().len(), 1);
}
