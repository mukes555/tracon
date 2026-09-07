import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import "./App.css";
import { api } from "./lib/api";
import { agentLabel, durationLabel, matchesKindFilter, matchesQuery, projectName, relTime } from "./lib/format";
import { applyTheme, normalizeTheme, THEME_KEY } from "./lib/theme";
import { useMediaQuery } from "./lib/useMediaQuery";
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
import { DetailPanel } from "./components/EventInspector";
import { InspectorPane } from "./components/Inspector";
import { EventList } from "./components/EventList";
import { FilterBar } from "./components/FilterBar";
import { FlaggedView } from "./components/FlaggedView";
import { LiveView } from "./components/LiveView";
import { NavRail } from "./components/NavRail";
import { KeyHints, StatusBar } from "./components/StatusBar";
import { TopBar } from "./components/TopBar";
import { OverviewView } from "./components/OverviewView";
import { PackagesView } from "./components/PackagesView";
import { SessionList } from "./components/SessionList";
import { SettingsView } from "./components/SettingsView";
import { ThreadViewer } from "./components/ThreadViewer";

const POLL_MS = 3000;
// Simple hides the operator details (raw commands, ids, rates); Advanced
// shows them everywhere. Persisted locally like a native app preference.
const ADVANCED_KEY = "tracon-advanced";
// Past this width the event detail docks as a right column instead of a
// slide-over, so the list stays visible while inspecting.
const INSPECTOR_QUERY = "(min-width: 1280px)";
const VIEW_KEYS: View[] = ["overview", "live", "timeline", "packages", "flagged", "settings"];
const TOAST_MS = 6000;

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
  const wide = useMediaQuery(INSPECTOR_QUERY);
  const [updatedAt, setUpdatedAt] = useState<string | null>(null);
  const [advanced, setAdvancedState] = useState(() => {
    try {
      return localStorage.getItem(ADVANCED_KEY) === "true";
    } catch {
      return false;
    }
  });
  const setAdvanced = (on: boolean) => {
    setAdvancedState(on);
    try {
      localStorage.setItem(ADVANCED_KEY, String(on));
    } catch {
      // preference just will not persist
    }
  };
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
  // Switching views also clears the inspector, so it never shows an event
  // from a list that is no longer on screen.
  const navigate = useCallback((v: View) => {
    setView(v);
    setDetail(null);
  }, []);
  const goSettings = useCallback(() => navigate("settings"), [navigate]);
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
  // One toast at a time, with an optional undo; a new one replaces the old.
  const [toast, setToast] = useState<{ text: string; undo?: () => void } | null>(null);
  const toastTimer = useRef<number | undefined>(undefined);
  const showToast = useCallback((text: string, undo?: () => void) => {
    window.clearTimeout(toastTimer.current);
    setToast({ text, undo });
    toastTimer.current = window.setTimeout(() => setToast(null), TOAST_MS);
  }, []);

  const ackMany = useCallback(
    async (targets: AgentEvent[], acked: boolean, withToast = true) => {
      const ids = targets.map((e) => e.id).filter((id): id is number => id !== undefined);
      await Promise.all(ids.map((id) => api.ackEvent(id, acked).catch(() => {})));
      flagsChanged();
      if (!withToast) return;
      const what = targets.length === 1 ? (targets[0].summary ?? "1 flag") : `${targets.length} flags`;
      showToast(`${acked ? "Acknowledged" : "Reopened"} ${what}`, () => ackMany(targets, !acked, false));
    },
    [flagsChanged, showToast],
  );
  const ackQuick = useCallback(
    (event: AgentEvent, acked = true) => ackMany([event], acked),
    [ackMany],
  );
  const ackFromPanel = useCallback(
    async (event: AgentEvent, acked: boolean) => {
      setDetail(null);
      await ackQuick(event, acked);
    },
    [ackQuick],
  );


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
        // The token also carries the live session count, so a session aging
        // out of the live window is itself a token change; the board and
        // nav badge decay on every view without extra polling.
        const token = await api.changeToken();
        setUpdatedAt(new Date().toISOString());
        const signature = `${view}|${selected}|${token.max_id}|${token.open_flags}|${token.acked_flags}|${token.live_sessions}`;
        if (signature === lastSignature.current) return;
        lastSignature.current = signature;

        // Independent reads run as one parallel round instead of a chain.
        const [nextStats, nextSessions, nextLive] = await Promise.all([
          api.stats(),
          api.sessions(),
          api.liveSessions(),
        ]);
        setStats(nextStats);
        setSessions(nextSessions);
        setLive(nextLive);

        if (view === "overview") {
          const [nextCapture, nextDays, nextFlagged, nextPackages] = await Promise.all([
            api.captureStatus(),
            api.eventsPerDay(),
            api.flaggedEvents(),
            api.packageEvents(),
          ]);
          setCapture(nextCapture);
          setDays(nextDays);
          setFlagged(nextFlagged);
          setPackages(nextPackages);
        }
        if (view === "packages") setPackages(await api.packageEvents());
        if (view === "flagged") setFlagged(await api.flaggedEvents());
        if (view === "live") {
          const pairs = await Promise.all(
            nextLive.map(async (s) => [s.session_id, await api.sessionTail(s.session_id)] as const),
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

  // Prev/next in the inspector walks the list the event was opened from.
  const detailList = view === "timeline" ? filteredEvents : view === "flagged" ? flagged : view === "packages" ? packages : [];
  const detailIndex = detail ? detailList.findIndex((e) => e.id === detail.event.id) : -1;
  const detailPosition = detailIndex >= 0 ? { index: detailIndex, total: detailList.length } : null;
  const stepDetail = useCallback(
    (delta: 1 | -1) => {
      const next = detailList[detailIndex + delta];
      if (next) setDetail({ event: next, acked: detail?.acked });
    },
    [detailList, detailIndex, detail?.acked],
  );
  // Arrow keys walk the current list; with nothing open they start at the
  // nearest end of it.
  const moveSelection = useCallback(
    (delta: 1 | -1) => {
      if (detailList.length === 0) return;
      if (detailIndex < 0) {
        setDetail({ event: detailList[delta > 0 ? 0 : detailList.length - 1] });
        return;
      }
      stepDetail(delta);
    },
    [detailList, detailIndex, stepDetail],
  );

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setPaletteOpen((open) => !open);
        return;
      }
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      if (paletteOpen || threadFor) return;
      const target = e.target as HTMLElement | null;
      const typing =
        target !== null &&
        (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable);
      if (typing) return;

      const viewNumber = Number(e.key);
      if (viewNumber >= 1 && viewNumber <= VIEW_KEYS.length) {
        navigate(VIEW_KEYS[viewNumber - 1]);
        return;
      }
      if (e.key === "ArrowDown" || e.key === "j") {
        e.preventDefault();
        moveSelection(1);
      } else if (e.key === "ArrowUp" || e.key === "k") {
        e.preventDefault();
        moveSelection(-1);
      } else if (e.key === "Escape") {
        setDetail(null);
      } else if (e.key === "a" && detail?.event.flag && !detail.acked) {
        ackQuick(detail.event, true);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [paletteOpen, threadFor, moveSelection, detail, ackQuick, navigate]);

  const readSelectedSession = useCallback(
    (s: SessionSummary) => setThreadFor({ sessionId: s.session_id, agent: s.agent, title: projectName(s.cwd) }),
    [],
  );

  return (
    <div className="shell">
      {paletteOpen && (
        <CommandPalette
          sessions={sessions}
          onNavigate={navigate}
          onOpenSession={openSessionTimeline}
          onOpenEvent={openDetail}
          onClose={() => setPaletteOpen(false)}
        />
      )}
      {detail && !wide && (
        <DetailPanel
          event={detail.event}
          acked={detail.acked}
          advanced={advanced}
          position={detailPosition}
          onStep={stepDetail}
          onClose={closeDetail}
          onReadThread={openThread}
          onAck={ackFromPanel}
          onOpenSession={openSessionTimeline}
        />
      )}
      {toast && (
        <div className="toast" role="status">
          <span>{toast.text}</span>
          {toast.undo && (
            <button
              onClick={() => {
                toast.undo?.();
                setToast(null);
              }}
            >
              Undo
            </button>
          )}
        </div>
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
      <NavRail view={view} stats={stats} liveCount={live.length} onNavigate={navigate} />

      <div className="stage">
      <TopBar
        advanced={advanced}
        onAdvanced={setAdvanced}
        onOpenPalette={() => setPaletteOpen(true)}
        onNavigate={navigate}
      />

      {view === "live" && (
        <LiveView
          sessions={live}
          tails={tails}
          advanced={advanced}
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
          advanced={advanced}
          onNavigate={navigate}
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
                    live={live.some((s) => s.session_id === selectedSession.session_id)}
                    onExport={exportSelected}
                    onReadThread={() =>
                      setThreadFor({
                        sessionId: selectedSession.session_id,
                        agent: selectedSession.agent,
                        title: projectName(selectedSession.cwd),
                      })
                    }
                  />
                  <FilterBar query={query} onQuery={setQuery} kind={kind} onKind={setKind} />
                  {exportNote && <p className="export-note">{exportNote}</p>}
                </div>
                {/* Key by session so the show-more window and expanded row
                    reset when the user switches sessions. */}
                <EventList
                  key={selectedSession.session_id}
                  events={filteredEvents}
                  showProject={false}
                  advanced={advanced}
                  selectedId={detail?.event.id}
                  onOpen={openDetail}
                />
                <StatusBar
                  left={`${filteredEvents.length} of ${events.length} events · updated ${updatedAt ? relTime(updatedAt) : "..."}`}
                  right={<KeyHints ack />}
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
          selectedId={detail?.event.id}
          onGoSettings={goSettings}
          onOpenEvent={openDetail}
        />
      )}

      {view === "flagged" && (
        <FlaggedView
          flagged={flagged}
          ackedCount={stats?.acked_count ?? 0}
          selectedId={detail?.event.id}
          onOpenEvent={openDetail}
          onAck={ackQuick}
          onAckMany={ackMany}
        />
      )}

      {view === "settings" && <SettingsView />}
      </div>
      {wide && (
        <InspectorPane
          event={detail?.event ?? null}
          acked={detail?.acked}
          advanced={advanced}
          position={detailPosition}
          onStep={stepDetail}
          context={{
            view,
            session: selectedSession,
            sessionEvents: events,
            stats,
            capture,
            liveCount: live.length,
            updatedAt,
            onOpenEvent: openDetail,
            onNavigate: navigate,
            onReadSession: readSelectedSession,
            onExportSession: exportSelected,
          }}
          onClose={closeDetail}
          onReadThread={openThread}
          onAck={ackFromPanel}
          onOpenSession={openSessionTimeline}
        />
      )}
    </div>
  );
}

function SessionHeader(props: {
  session: SessionSummary;
  live: boolean;
  onExport: () => void;
  onReadThread: () => void;
}) {
  const s = props.session;
  return (
    <div className="session-header">
      <div>
        <h1 className="session-title">
          {props.live && <span className="pulse-dot" title="Active in the last 5 minutes" />}
          {projectName(s.cwd)}
        </h1>
        <p className="view-sub">
          {agentLabel(s.agent)} · {s.event_count} events · {s.command_count} commands ·{" "}
          {durationLabel(s.started_at, s.last_at)}
          {s.hook_tool_count > 0 && s.tail_tool_count > 0 && (
            <span
              className="gap-chip"
              title="Hooks were off for part of this session, or Tracon was not running. Those tool calls were recovered from the transcript."
            >
              {s.tail_tool_count} recovered from transcript
            </span>
          )}
        </p>
      </div>
      <div className="session-actions">
        <button className="btn-dark" onClick={props.onReadThread}>
          Conversation
        </button>
        <button className="ack-btn" onClick={props.onExport}>
          Export JSON
        </button>
      </div>
    </div>
  );
}

export default App;
