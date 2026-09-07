import type { AgentEvent } from "../../lib/types";

/** A plain tool_call event; override only the fields a test cares about. */
export function makeEvent(overrides: Partial<AgentEvent> = {}): AgentEvent {
  return {
    agent: "claude-code",
    session_id: "session-1",
    ts: "2026-09-08T10:00:00.000Z",
    kind: "tool_call",
    source: "hook",
    cwd: null,
    tool_name: null,
    summary: null,
    flag: null,
    payload: null,
    ...overrides,
  };
}
