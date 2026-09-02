import type { AgentEvent, LiveSession } from "../lib/types";
import { agentLabel, projectName, relTime, timeOf } from "../lib/format";
import { CamIcon } from "./icons";

/// The security room: one monitor per active session, each streaming its
/// recent events. Rows open the slide-over; the footer jumps to the full
/// timeline or the conversation.
export function LiveView(props: {
  sessions: LiveSession[];
  tails: Record<string, AgentEvent[]>;
  onOpenSession: (sessionId: string) => void;
  onOpenEvent: (event: AgentEvent) => void;
  onReadThread: (session: LiveSession) => void;
}) {
  return (
    <main className="view">
      <header className="view-head">
        <h1>Live</h1>
        <p className="view-sub">
          Every active agent session on this machine, streaming as it works.
        </p>
      </header>

      {props.sessions.length === 0 ? (
        <div className="pkg-empty">
          <CamIcon size={34} />
          <p>All quiet. No agents are working right now.</p>
          <p className="muted">
            Start a Claude Code, Codex, Cursor, or Gemini session and its
            monitor appears here within seconds.
          </p>
        </div>
      ) : (
        <div className="cam-grid">
          {props.sessions.map((s) => {
            const tail = props.tails[s.session_id] ?? [];
            const overflow = s.subagent_count - s.subagents.length;
            return (
              <section
                key={s.session_id}
                className={s.flagged_count > 0 ? "cam-card flagged" : "cam-card"}
              >
                <header className="cam-head">
                  <span className="pulse-dot" />
                  <b className="cam-project">{projectName(s.cwd)}</b>
                  <span className={`agent-chip agent-${s.agent}`}>{agentLabel(s.agent)}</span>
                  {s.flagged_count > 0 && (
                    <span className="flag-chip">{s.flagged_count} flagged</span>
                  )}
                  <span className="cam-time">{relTime(s.last_ts)}</span>
                </header>

                {s.last_prompt && <p className="cam-task">{s.last_prompt}</p>}
                {s.subagents.length > 0 && (
                  <p className="cam-subagents">
                    running: {s.subagents.join(" · ")}
                    {overflow > 0 && ` · +${overflow} more`}
                  </p>
                )}

                <ul className="cam-screen">
                  {tail.map((e, i) => (
                    <li key={e.id ?? i}>
                      <button
                        className={e.flag ? "cam-row bad" : "cam-row"}
                        onClick={() => props.onOpenEvent(e)}
                      >
                        <span className="cam-ts">{timeOf(e.ts)}</span>
                        <span className="cam-kind">{e.tool_name ?? e.kind}</span>
                        <span className="cam-sum">{e.summary}</span>
                      </button>
                    </li>
                  ))}
                  {tail.length === 0 && <li className="cam-static">connecting feed...</li>}
                </ul>

                <footer className="cam-foot">
                  <button className="thread-btn" onClick={() => props.onReadThread(s)}>
                    Conversation
                  </button>
                  <button className="ack-btn" onClick={() => props.onOpenSession(s.session_id)}>
                    Timeline
                  </button>
                </footer>
              </section>
            );
          })}
        </div>
      )}
    </main>
  );
}
