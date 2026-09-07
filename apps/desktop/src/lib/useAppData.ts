import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "./api";
import type {
  AgentEvent,
  CaptureStatus,
  DayCount,
  LiveSession,
  SessionSummary,
  Stats,
  View,
} from "./types";

const POLL_MS = 3000;

export type Connection = "ok" | "lost";

/// Everything the UI reads from the recorder, kept fresh by one poll.
/// Each tick fetches only the change token (a MAX(id) plus a few tiny
/// counts); stats and the heavier lists refetch when data actually changed
/// or the view did, so an idle app costs SQLite almost nothing.
export function useAppData(view: View, selected: string | null) {
  const [stats, setStats] = useState<Stats | null>(null);
  const [sessions, setSessions] = useState<SessionSummary[]>([]);
  const [events, setEvents] = useState<AgentEvent[]>([]);
  const [packages, setPackages] = useState<AgentEvent[]>([]);
  const [flagged, setFlagged] = useState<AgentEvent[]>([]);
  const [days, setDays] = useState<DayCount[]>([]);
  const [live, setLive] = useState<LiveSession[]>([]);
  const [tails, setTails] = useState<Record<string, AgentEvent[]>>({});
  const [capture, setCapture] = useState<CaptureStatus | null>(null);
  const [updatedAt, setUpdatedAt] = useState<string | null>(null);
  const [connection, setConnection] = useState<Connection>("ok");
  const [paused, setPaused] = useState(false);
  // Bumped whenever flags change through the UI, so views holding their own
  // flag lists (the acknowledged tab) know to refetch without depending on
  // array identity.
  const [flagsVersion, setFlagsVersion] = useState(0);

  const lastSignature = useRef("");
  const refreshRef = useRef<() => void>(() => {});

  useEffect(() => {
    // A slow reply from a previous view or session must never land on top
    // of the current one; the cleanup flips this and every await checks it.
    let cancelled = false;
    const refresh = async () => {
      try {
        // The token also carries the live session count, so a session aging
        // out of the live window is itself a token change; the board and
        // nav badge decay on every view without extra polling.
        const token = await api.changeToken();
        if (cancelled) return;
        setPaused(token.paused);
        setConnection("ok");
        const signature = `${view}|${selected}|${token.max_id}|${token.open_flags}|${token.acked_flags}|${token.live_sessions}`;
        if (signature === lastSignature.current) return;

        // Independent reads run as one parallel round instead of a chain.
        const [nextStats, nextSessions, nextLive] = await Promise.all([
          api.stats(),
          api.sessions(),
          api.liveSessions(),
        ]);
        if (cancelled) return;
        setUpdatedAt(new Date().toISOString());
        setStats(nextStats);
        setSessions(nextSessions);
        setLive(nextLive);

        if (view === "overview" || view === "settings") {
          const nextCapture = await api.captureStatus();
          if (cancelled) return;
          setCapture(nextCapture);
        }
        if (view === "overview") {
          const [nextDays, nextFlagged, nextPackages] = await Promise.all([
            api.eventsPerDay(),
            api.flaggedEvents(),
            api.packageEvents(),
          ]);
          if (cancelled) return;
          setDays(nextDays);
          setFlagged(nextFlagged);
          setPackages(nextPackages);
        }
        if (view === "packages") {
          const nextPackages = await api.packageEvents();
          if (cancelled) return;
          setPackages(nextPackages);
        }
        if (view === "flagged") {
          const nextFlagged = await api.flaggedEvents();
          if (cancelled) return;
          setFlagged(nextFlagged);
        }
        if (view === "live") {
          const pairs = await Promise.all(
            nextLive.map(async (s) => [s.session_id, await api.sessionTail(s.session_id)] as const),
          );
          if (cancelled) return;
          setTails(Object.fromEntries(pairs));
        }
        if (view === "timeline" && selected) {
          const nextEvents = await api.sessionEvents(selected);
          if (cancelled) return;
          setEvents(nextEvents);
        }
        // Only a fully applied round counts; a failed one retries next tick.
        lastSignature.current = signature;
      } catch {
        if (!cancelled) setConnection("lost");
      }
    };
    refreshRef.current = refresh;
    refresh();
    const timer = setInterval(refresh, POLL_MS);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, [view, selected]);

  const flagsChanged = useCallback(async () => {
    try {
      const [nextStats, nextFlagged] = await Promise.all([api.stats(), api.flaggedEvents()]);
      setStats(nextStats);
      setFlagged(nextFlagged);
      setFlagsVersion((v) => v + 1);
    } catch {
      // next poll refreshes
    }
  }, []);

  /// Forces a full refetch on the next tick and starts one now (after a
  /// purge, the lists must not wait for the poll).
  const refreshNow = useCallback(() => {
    lastSignature.current = "";
    refreshRef.current();
  }, []);

  return {
    stats,
    sessions,
    events,
    packages,
    flagged,
    days,
    live,
    tails,
    capture,
    updatedAt,
    connection,
    paused,
    setPaused,
    flagsVersion,
    flagsChanged,
    refreshNow,
  };
}
