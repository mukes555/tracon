import { invoke } from "@tauri-apps/api/core";
import type { AgentEvent, CaptureStatus, ChangeToken, DayCount, LiveSession, SessionSummary, Stats, ThreadMessage } from "./types";

export const api = {
  changeToken: () => invoke<ChangeToken>("change_token"),
  stats: () => invoke<Stats>("stats"),
  sessions: () => invoke<SessionSummary[]>("sessions"),
  sessionEvents: (sessionId: string) => invoke<AgentEvent[]>("session_events", { sessionId }),
  packageEvents: () => invoke<AgentEvent[]>("package_events"),
  flaggedEvents: (acked = false) => invoke<AgentEvent[]>("flagged_events", { acked }),
  ackEvent: (id: number, acked: boolean) => invoke("ack_event", { id, acked }),
  liveSessions: () => invoke<LiveSession[]>("live_sessions"),
  searchEvents: (query: string) => invoke<AgentEvent[]>("search_events", { query }),
  sessionThread: (sessionId: string) => invoke<ThreadMessage[]>("session_thread", { sessionId }),
  captureStatus: () => invoke<CaptureStatus>("capture_status"),
  eventsPerDay: () => invoke<DayCount[]>("events_per_day"),
  eventPayload: (id: number) => invoke<unknown>("event_payload", { id }),
  exportSession: (sessionId: string) => invoke<string>("export_session", { sessionId }),
  dataDir: () => invoke<string>("data_dir"),
  importFullHistory: () => invoke<string>("import_full_history"),
  getSetting: (key: string) => invoke<string | null>("get_setting", { key }),
  setSetting: (key: string, value: string) => invoke("set_setting", { key, value }),
};
