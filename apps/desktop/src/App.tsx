import { useCallback, useEffect, useMemo, useState } from "react";
import "./App.css";
import { api } from "./lib/api";
import { matchesKindFilter, matchesQuery, projectName } from "./lib/format";
import { useAppData } from "./lib/useAppData";
import { useToast } from "./lib/useToast";
import { useUpdateStatus } from "./lib/useUpdateStatus";
import { useTransientNote } from "./lib/useTransientNote";
import { applyTheme, normalizeTheme, THEME_KEY } from "./lib/theme";
import { useMediaQuery } from "./lib/useMediaQuery";
import type { AgentEvent, KindFilter, View } from "./lib/types";
import { CommandPalette } from "./components/CommandPalette";
import { DetailPanel } from "./components/EventInspector";
import { type InspectorContext, InspectorPane } from "./components/Inspector";
import { FlaggedView } from "./components/FlaggedView";
import { LiveView } from "./components/LiveView";
import { NavRail } from "./components/NavRail";
import { StatusBanners } from "./components/StatusBanners";
import { TopBar } from "./components/TopBar";
import { OverviewView } from "./components/OverviewView";
import { PackagesView } from "./components/PackagesView";
import { SettingsView } from "./components/SettingsView";
import { TimelineView } from "./components/TimelineView";
import { ThreadViewer } from "./components/ThreadViewer";

// Simple hides the operator details (raw commands, ids, rates); Advanced
// shows them everywhere. Persisted locally like a native app preference.
const ADVANCED_KEY = "tracon-advanced";
// Past this width the event detail docks as a right column instead of a
// slide-over, so the list stays visible while inspecting.
const INSPECTOR_QUERY = "(min-width: 1280px)";
// Only the list views have something to inspect. Overview, Live and
// Settings own their full width instead of carrying an empty column.
const INSPECTOR_VIEWS: View[] = ["timeline", "packages", "flagged"];
const VIEW_KEYS: View[] = ["overview", "live", "timeline", "packages", "flagged", "settings"];
const EXPORT_NOTE_MS = 6000;

// The rows the current list actually rendered, in display order. Keyboard
// stepping and prev/next walk this, never a filtered-out or folded row.
type Cursor = { events: AgentEvent[]; acked: boolean };
const NO_CURSOR: Cursor = { events: [], acked: false };

function App() {
  const [view, setView] = useState<View>("overview");
  const [selected, setSelected] = useState<string | null>(null);
  const data = useAppData(view, selected);
  const [intelEnabled, setIntelEnabled] = useState<boolean | null>(null);
  const [query, setQuery] = useState("");
  const [kind, setKind] = useState<KindFilter>("all");
  const { note: exportNote, show: showExportNote } = useTransientNote(EXPORT_NOTE_MS);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const wide = useMediaQuery(INSPECTOR_QUERY);
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
  const update = useUpdateStatus();

  const { flagsChanged, refreshNow, setPaused } = data;
  const ackMany = useCallback(
    async (targets: AgentEvent[], acked: boolean, withToast = true) => {
      const ids = targets.map((e) => e.id).filter((id): id is number => id !== undefined);
      if (ids.length === 0) return;
      try {
        await api.ackEvents(ids, acked);
      } catch {
        showToast("Could not update flags.", { label: "Retry", run: () => ackMany(targets, acked, withToast) });
        return;
      }
      flagsChanged();
      if (!withToast) return;
      const what = targets.length === 1 ? (targets[0].summary ?? "1 flag") : `${targets.length} flags`;
      showToast(`${acked ? "Acknowledged" : "Reopened"} ${what}`, {
        label: "Undo",
        run: () => ackMany(targets, !acked, false),
      });
    },
    [flagsChanged, showToast],
  );
  const ackQuick = useCallback(
    (event: AgentEvent, acked = true) => ackMany([event], acked),
    [ackMany],
  );

  const setCapturePaused = useCallback(
    async (paused: boolean) => {
      await api.setCapturePaused(paused);
      setPaused(paused);
    },
    [setPaused],
  );
  const resumeCapture = useCallback(() => {
    setCapturePaused(false).catch(() => showToast("Could not resume capture"));
  }, [setCapturePaused, showToast]);

  const deleteSession = useCallback(
    async (sessionId: string) => {
      try {
        const removed = await api.purgeSession(sessionId);
        setSelected((current) => (current === sessionId ? null : current));
        setDetail(null);
        showToast(`Deleted ${removed.toLocaleString()} events`);
        refreshNow();
      } catch {
        showToast("Could not delete the session");
      }
    },
    [refreshNow, showToast],
  );
  const deleteEverything = useCallback(async () => {
    try {
      const removed = await api.purgeAll();
      setSelected(null);
      setDetail(null);
      showToast(`Deleted ${removed.toLocaleString()} events`);
      refreshNow();
    } catch {
      showToast("Could not delete the data");
    }
  }, [refreshNow, showToast]);

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

  const exportSelected = useCallback(async () => {
    if (!selected) return;
    try {
      const path = await api.exportSession(selected);
      showExportNote(`Exported to ${path}`);
    } catch (err) {
      showExportNote(`Export failed: ${String(err)}`, "bad");
    }
  }, [selected, showExportNote]);

  const filteredEvents = useMemo(
    () => data.events.filter((e) => matchesKindFilter(e, kind) && matchesQuery(e, query)),
    [data.events, kind, query],
  );

  const selectedSession = data.sessions.find((s) => s.session_id === selected) ?? null;

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
      const target = e.target as HTMLElement | null;
      const typing =
        target !== null &&
        (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable);
      // Escape inside a search box only leaves the box; a second Escape
      // closes the inspector.
      if (e.key === "Escape") {
        if (typing) target.blur();
        else setDetail(null);
        return;
      }
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

  // The docked inspector's context only changes when one of these does, so
  // the polled lists it does not read never re-render it.
  const inspectorContext = useMemo<InspectorContext>(
    () => ({
      view,
      session: selectedSession,
      sessionEvents: data.events,
      onOpenEvent: openDetail,
      onReadSession: openSessionThread,
      onExportSession: exportSelected,
      onDeleteSession: deleteSession,
    }),
    [
      view,
      selectedSession,
      data.events,
      openDetail,
      openSessionThread,
      exportSelected,
      deleteSession,
    ],
  );

  return (
    <div className="shell">
      {paletteOpen && (
        <CommandPalette
          sessions={data.sessions}
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
          {toast.action && (
            <button
              onClick={() => {
                toast.action?.run();
                dismissToast();
              }}
            >
              {toast.action.label}
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
      <NavRail
        view={view}
        stats={data.stats}
        liveCount={data.live.length}
        paused={data.paused}
        onNavigate={navigate}
      />

      <div className="stage">
      <TopBar
        advanced={advanced}
        onAdvanced={setAdvanced}
        onOpenPalette={() => setPaletteOpen(true)}
        onNavigate={navigate}
      />
      <StatusBanners
        connection={data.connection}
        paused={data.paused}
        onResume={resumeCapture}
        update={update.status}
      />

      {view === "live" && (
        <LiveView
          sessions={data.live}
          tails={data.tails}
          advanced={advanced}
          onOpenSession={openSessionTimeline}
          onOpenEvent={openDetail}
          onReadThread={openSessionThread}
        />
      )}

      {view === "overview" && (
        <OverviewView
          stats={data.stats}
          days={data.days}
          capture={data.capture}
          recentFlagged={data.flagged}
          recentPackages={data.packages}
          liveSessions={data.live}
          advanced={advanced}
          onNavigate={navigate}
          onOpenEvent={openDetail}
          onAck={ackQuick}
          onOpenSession={openSessionTimeline}
        />
      )}

      {view === "timeline" && (
        <TimelineView
          sessions={data.sessions}
          selected={selected}
          session={selectedSession}
          live={selectedSession !== null && data.live.some((l) => l.session_id === selectedSession.session_id)}
          events={data.events}
          filteredEvents={filteredEvents}
          query={query}
          kind={kind}
          exportNote={exportNote?.text ?? null}
          updatedAt={data.updatedAt}
          advanced={advanced}
          selectedId={detail?.event.id}
          onSelect={setSelected}
          onQuery={setQuery}
          onKind={setKind}
          onExport={exportSelected}
          onDelete={deleteSession}
          onReadThread={openSessionThread}
          onOpen={openDetail}
          onVisibleRows={onVisibleRows}
        />
      )}

      {view === "packages" && (
        <PackagesView
          packages={data.packages}
          intelEnabled={intelEnabled}
          selectedId={detail?.event.id}
          onGoSettings={goSettings}
          onOpenEvent={openDetail}
          onVisibleRows={onVisibleRows}
        />
      )}

      {view === "flagged" && (
        <FlaggedView
          flagged={data.flagged}
          ackedCount={data.stats?.acked_count ?? 0}
          flagsVersion={data.flagsVersion}
          selectedId={detail?.event.id}
          onOpenEvent={openDetail}
          onAck={ackQuick}
          onAckMany={ackMany}
          onVisibleRows={onVisibleRows}
        />
      )}

      {view === "settings" && (
        <SettingsView
          paused={data.paused}
          onSetPaused={setCapturePaused}
          capture={data.capture}
          eventCount={data.stats?.event_count ?? 0}
          onDeleteAll={deleteEverything}
          update={update.status}
          onSetUpdateCheck={update.setEnabled}
          onCheckUpdate={update.checkNow}
        />
      )}
      </div>
      {wide && INSPECTOR_VIEWS.includes(view) && (
        <InspectorPane
          event={detail?.event ?? null}
          acked={detail?.acked}
          advanced={advanced}
          position={detailPosition}
          onStep={stepDetail}
          context={inspectorContext}
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
