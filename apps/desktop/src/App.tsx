import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import "./App.css";
import { api } from "./lib/api";
import { agentLabel, matchesKindFilter, matchesQuery, projectName } from "./lib/format";
import { applyTheme, normalizeTheme, THEME_KEY } from "./lib/theme";
import type {
  AgentEvent,
  CaptureStatus,
  DayCount,
  KindFilter,
  LiveSession,
  SessionSummary,
  Stats,
  View,
} from "./lib/types";
import { CommandPalette } from "./components/CommandPalette";
import { DetailPanel } from "./components/DetailPanel";
import { EventList } from "./components/EventList";
import { FilterBar } from "./components/FilterBar";
import { FlaggedView } from "./components/FlaggedView";
import { LiveView } from "./components/LiveView";
import { NavRail } from "./components/NavRail";
import { OverviewView } from "./components/OverviewView";
import { PackagesView } from "./components/PackagesView";
import { SessionList } from "./components/SessionList";
import { SettingsView } from "./components/SettingsView";
import { ThreadViewer } from "./components/ThreadViewer";

const POLL_MS = 3000;

function App() {
  const [view, setView] = useState<View>("overview");
  const [stats, setStats] = useState<Stats | null>(null);
  const [sessions, setSessions] = useState<SessionSummary[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [events, setEvents] = useState<AgentEvent[]>([]);
  const [packages, setPackages] = useState<AgentEvent[]>([]);
  const [flagged, setFlagged] = useState<AgentEvent[]>([]);
  const [days, setDays] = useState<DayCount[]>([]);
  const [live, setLive] = useState<LiveSession[]>([]);
  const [tails, setTails] = useState<Record<string, AgentEvent[]>>({});
  const [capture, setCapture] = useState<CaptureStatus | null>(null);
  const [intelEnabled, setIntelEnabled] = useState<boolean | null>(null);
  const [query, setQuery] = useState("");
  const [kind, setKind] = useState<KindFilter>("all");
  const [exportNote, setExportNote] = useState<string | null>(null);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [threadFor, setThreadFor] = useState<{
    sessionId: string;
    ts?: string;
    agent?: string;
    title?: string;
  } | null>(null);

  // Stable identities keep the memoized ThreadViewer from re-rendering its
  // message list on every 3s stats poll.
  const openThread = useCallback((event: AgentEvent) => {
    setThreadFor({
      sessionId: event.session_id,
      ts: event.ts,
      agent: event.agent,
      title: projectName(event.cwd),
    });
  }, []);
  const closeThread = useCallback(() => setThreadFor(null), []);
  const openSessionThread = useCallback((s: LiveSession) => {
    setThreadFor({ sessionId: s.session_id, agent: s.agent, title: projectName(s.cwd) });
  }, []);
  const goSettings = useCallback(() => setView("settings"), []);
  const flagsChanged = useCallback(async () => {
    try {
      setStats(await api.stats());
      setFlagged(await api.flaggedEvents());
    } catch {
      // next poll refreshes
    }
  }, []);

  // Mission-control drill-down: any row anywhere opens the same slide-over.
  const [detail, setDetail] = useState<{ event: AgentEvent; acked?: boolean } | null>(null);
  const openDetail = useCallback((event: AgentEvent, acked?: boolean) => {
    setDetail({ event, acked });
  }, []);
  const closeDetail = useCallback(() => setDetail(null), []);
  const openSessionTimeline = useCallback((sessionId: string) => {
    setView("timeline");
    setSelected(sessionId);
    setDetail(null);
  }, []);
  const ackQuick = useCallback(
    async (event: AgentEvent, acked = true) => {
      if (event.id !== undefined) {
        await api.ackEvent(event.id, acked).catch(() => {});
      }
      flagsChanged();
    },
    [flagsChanged],
  );
  const ackFromPanel = useCallback(
    async (event: AgentEvent, acked: boolean) => {
      setDetail(null);
      await ackQuick(event, acked);
    },
    [ackQuick],
  );

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setPaletteOpen((open) => !open);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  useEffect(() => {
    api
      .getSetting(THEME_KEY)
      .then((v) => applyTheme(normalizeTheme(v)))
      .catch(() => applyTheme("system"));
    api
      .getSetting("threat_intel_enabled")
      .then((v) => setIntelEnabled(v === "true"))
      .catch(() => setIntelEnabled(false));
  }, []);

  // Each poll fetches only the change token (a MAX(id) plus two tiny counts);
  // stats and the heavier lists refetch when data actually changed or the
  // view did, so an idle app costs SQLite almost nothing.
  const lastSignature = useRef("");
  useEffect(() => {
    const refresh = async () => {
      try {
        // The live board must decay on its own (sessions drop out after
        // five quiet minutes with no new events to bump the token), so it
        // refreshes every poll while watched.
        if (view === "live") {
          setLive(await api.liveSessions());
        }
        const token = await api.changeToken();
        const signature = `${view}|${selected}|${token.max_id}|${token.open_flags}|${token.acked_flags}`;
        if (signature === lastSignature.current) return;
        lastSignature.current = signature;

        setStats(await api.stats());
        setSessions(await api.sessions());
        if (view === "overview") {
          setCapture(await api.captureStatus());
          setDays(await api.eventsPerDay());
          setLive(await api.liveSessions());
          setFlagged(await api.flaggedEvents());
          setPackages(await api.packageEvents());
        }
        if (view === "packages") setPackages(await api.packageEvents());
        if (view === "flagged") setFlagged(await api.flaggedEvents());
        if (view === "live") {
          const sessions = await api.liveSessions();
          setLive(sessions);
          const pairs = await Promise.all(
            sessions.map(async (s) => [s.session_id, await api.sessionTail(s.session_id)] as const),
          );
          setTails(Object.fromEntries(pairs));
        }
        if (view === "timeline" && selected) {
          setEvents(await api.sessionEvents(selected));
        }
      } catch {
        // Backend not ready yet; next poll retries.
      }
    };
    refresh();
    const timer = setInterval(refresh, POLL_MS);
    return () => clearInterval(timer);
  }, [view, selected]);

  const exportSelected = async () => {
    if (!selected) return;
    try {
      const path = await api.exportSession(selected);
      setExportNote(`Exported to ${path}`);
    } catch (err) {
      setExportNote(`Export failed: ${String(err)}`);
    }
    setTimeout(() => setExportNote(null), 6000);
  };

  const filteredEvents = useMemo(
    () => events.filter((e) => matchesKindFilter(e, kind) && matchesQuery(e, query)),
    [events, kind, query],
  );

  const selectedSession = sessions.find((s) => s.session_id === selected) ?? null;

  return (
    <div className="shell">
      {paletteOpen && (
        <CommandPalette
          sessions={sessions}
          onNavigate={setView}
          onOpenSession={openSessionTimeline}
          onOpenEvent={openDetail}
          onClose={() => setPaletteOpen(false)}
        />
      )}
      {detail && (
        <DetailPanel
          event={detail.event}
          acked={detail.acked}
          onClose={closeDetail}
          onReadThread={openThread}
          onAck={ackFromPanel}
          onOpenSession={openSessionTimeline}
        />
      )}
      {threadFor && (
        <ThreadViewer
          sessionId={threadFor.sessionId}
          targetTs={threadFor.ts}
          agent={threadFor.agent}
          title={threadFor.title}
          onClose={closeThread}
        />
      )}
      <NavRail
        view={view}
        stats={stats}
        liveCount={live.length}
        onNavigate={setView}
        onOpenPalette={() => setPaletteOpen(true)}
      />

      {view === "live" && (
        <LiveView
          sessions={live}
          tails={tails}
          onOpenSession={openSessionTimeline}
          onOpenEvent={openDetail}
          onReadThread={openSessionThread}
        />
      )}

      {view === "overview" && (
        <OverviewView
          stats={stats}
          days={days}
          capture={capture}
          recentFlagged={flagged}
          recentPackages={packages}
          liveSessions={live}
          onNavigate={setView}
          onOpenEvent={openDetail}
          onAck={ackQuick}
          onOpenSession={openSessionTimeline}
        />
      )}

      {view === "timeline" && (
        <div className="body">
          <aside className="sessions">
            <SessionList sessions={sessions} selected={selected} onSelect={setSelected} />
          </aside>
          <main className="timeline">
            {selectedSession ? (
              <>
                <div className="timeline-toolbar">
                  <SessionHeader
                    session={selectedSession}
                    onExport={exportSelected}
                    onReadThread={() =>
                      setThreadFor({
                        sessionId: selectedSession.session_id,
                        agent: selectedSession.agent,
                        title: projectName(selectedSession.cwd),
                      })
                    }
                  />
                  {selectedSession.hook_tool_count > 0 && selectedSession.tail_tool_count > 0 && (
                    <p className="gap-banner">
                      Capture gap: {selectedSession.tail_tool_count} of this session's
                      tool calls were recovered from the transcript only. Hooks were
                      disabled for part of the session, or Tracon wasn't running.
                    </p>
                  )}
                  <FilterBar query={query} onQuery={setQuery} kind={kind} onKind={setKind} />
                  {exportNote && <p className="export-note">{exportNote}</p>}
                </div>
                {/* Key by session so the show-more window and expanded row
                    reset when the user switches sessions. */}
                <EventList
                  key={selectedSession.session_id}
                  events={filteredEvents}
                  showProject={false}
                  onOpen={openDetail}
                />
              </>
            ) : (
              <div className="pkg-empty">
                <p>Select a session to see its timeline.</p>
                <p className="muted">
                  Every command, file edit, and install, in the order it happened.
                </p>
              </div>
            )}
          </main>
        </div>
      )}

      {view === "packages" && (
        <PackagesView
          packages={packages}
          intelEnabled={intelEnabled}
          onGoSettings={goSettings}
          onOpenEvent={openDetail}
        />
      )}

      {view === "flagged" && (
        <FlaggedView
          flagged={flagged}
          ackedCount={stats?.acked_count ?? 0}
          onOpenEvent={openDetail}
          onChanged={flagsChanged}
        />
      )}

      {view === "settings" && <SettingsView />}
    </div>
  );
}

function SessionHeader(props: {
  session: SessionSummary;
  onExport: () => void;
  onReadThread: () => void;
}) {
  const s = props.session;
  return (
    <div className="session-header">
      <div>
        <h1>{projectName(s.cwd)}</h1>
        <p className="view-sub">
          {agentLabel(s.agent)} · {s.event_count} events · {s.command_count} commands ·{" "}
          {durationLabel(s.started_at, s.last_at)}
        </p>
      </div>
      <div className="session-actions">
        <button className="btn-dark" onClick={props.onReadThread}>
          Conversation
        </button>
        <button className="btn-dark" onClick={props.onExport}>
          Export JSON
        </button>
      </div>
    </div>
  );
}

function durationLabel(start: string, end: string): string {
  const ms = new Date(end).getTime() - new Date(start).getTime();
  if (!Number.isFinite(ms) || ms < 0) return "";
  const mins = Math.round(ms / 60000);
  if (mins < 1) return "under a minute";
  if (mins < 60) return `${mins} min`;
  return `${Math.floor(mins / 60)}h ${mins % 60}m`;
}

export default App;
