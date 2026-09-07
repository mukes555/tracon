import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import "./App.css";
import { api } from "./lib/api";
import { matchesKindFilter, matchesQuery, projectName } from "./lib/format";
import { useToast } from "./lib/useToast";
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
import { FlaggedView } from "./components/FlaggedView";
import { LiveView } from "./components/LiveView";
import { NavRail } from "./components/NavRail";
import { TopBar } from "./components/TopBar";
import { OverviewView } from "./components/OverviewView";
import { PackagesView } from "./components/PackagesView";
import { SettingsView } from "./components/SettingsView";
import { TimelineView } from "./components/TimelineView";
import { ThreadViewer } from "./components/ThreadViewer";

const POLL_MS = 3000;
// Simple hides the operator details (raw commands, ids, rates); Advanced
// shows them everywhere. Persisted locally like a native app preference.
const ADVANCED_KEY = "tracon-advanced";
// Past this width the event detail docks as a right column instead of a
// slide-over, so the list stays visible while inspecting.
const INSPECTOR_QUERY = "(min-width: 1280px)";
const VIEW_KEYS: View[] = ["overview", "live", "timeline", "packages", "flagged", "settings"];

// The rows the current list actually rendered, in display order. Keyboard
// stepping and prev/next walk this, never a filtered-out or folded row.
type Cursor = { events: AgentEvent[]; acked: boolean };
const NO_CURSOR: Cursor = { events: [], acked: false };

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
  const openSessionThread = useCallback((s: { session_id: string; agent: string; cwd: string | null }) => {
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
  const { toast, show: showToast, dismiss: dismissToast } = useToast();

  const ackMany = useCallback(
    async (targets: AgentEvent[], acked: boolean, withToast = true) => {
      const ids = targets.map((e) => e.id).filter((id): id is number => id !== undefined);
      if (ids.length === 0) return;
      await api.ackEvents(ids, acked).catch(() => {});
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

  const [cursor, setCursor] = useState<Cursor>(NO_CURSOR);
  const onVisibleRows = useCallback((events: AgentEvent[], acked = false) => {
    setCursor(events.length === 0 ? NO_CURSOR : { events, acked });
  }, []);
  const detailIndex = useMemo(
    () => (detail ? cursor.events.findIndex((e) => e.id === detail.event.id) : -1),
    [cursor, detail],
  );
  const detailPosition = detailIndex >= 0 ? { index: detailIndex, total: cursor.events.length } : null;
  const stepDetail = useCallback(
    (delta: 1 | -1) => {
      const next = cursor.events[detailIndex + delta];
      if (next) setDetail({ event: next, acked: cursor.acked });
    },
    [cursor, detailIndex],
  );
  // Arrow keys walk the rendered rows; with nothing open they start at the
  // nearest end.
  const moveSelection = useCallback(
    (delta: 1 | -1) => {
      const rows = cursor.events;
      if (rows.length === 0) return;
      if (detailIndex < 0) {
        setDetail({ event: rows[delta > 0 ? 0 : rows.length - 1], acked: cursor.acked });
        return;
      }
      stepDetail(delta);
    },
    [cursor, detailIndex, stepDetail],
  );
  // Acknowledging from the inspector keeps the triage flowing: the docked
  // column moves to the next row, the slide-over closes.
  const ackFromPanel = useCallback(
    async (event: AgentEvent, acked: boolean) => {
      const next = cursor.events[detailIndex + 1] ?? cursor.events[detailIndex - 1];
      if (!wide) setDetail(null);
      else if (next) setDetail({ event: next, acked: cursor.acked });
      else setDetail({ event, acked });
      await ackQuick(event, acked);
    },
    [cursor, detailIndex, wide, ackQuick],
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
      if (e.key === "Escape") {
        setDetail(null);
        return;
      }
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
      } else if (e.key === "a" && detail?.event.flag && !detail.acked) {
        ackFromPanel(detail.event, true);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [paletteOpen, threadFor, moveSelection, detail, ackFromPanel, navigate]);

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
                dismissToast();
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
        <TimelineView
          sessions={sessions}
          selected={selected}
          session={selectedSession}
          live={selectedSession !== null && live.some((l) => l.session_id === selectedSession.session_id)}
          events={events}
          filteredEvents={filteredEvents}
          query={query}
          kind={kind}
          exportNote={exportNote}
          updatedAt={updatedAt}
          advanced={advanced}
          selectedId={detail?.event.id}
          onSelect={setSelected}
          onQuery={setQuery}
          onKind={setKind}
          onExport={exportSelected}
          onReadThread={openSessionThread}
          onOpen={openDetail}
          onVisibleRows={onVisibleRows}
        />
      )}

      {view === "packages" && (
        <PackagesView
          packages={packages}
          intelEnabled={intelEnabled}
          selectedId={detail?.event.id}
          onGoSettings={goSettings}
          onOpenEvent={openDetail}
          onVisibleRows={onVisibleRows}
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
          onVisibleRows={onVisibleRows}
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
            onReadSession: openSessionThread,
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

export default App;
