import type { AgentEvent, SessionSummary, View } from "../lib/types";
import { agentLabel, durationLabel, hasTranscript, projectName, relTime } from "../lib/format";
import { ConfirmButton } from "./ConfirmButton";
import { EventInspector, type EventInspectorProps } from "./EventInspector";

export type InspectorContext = {
  view: View;
  session: SessionSummary | null;
  sessionEvents: AgentEvent[];
  onOpenEvent: (event: AgentEvent) => void;
  onReadSession: (session: SessionSummary) => void;
  onExportSession: () => void;
  onDeleteSession: (sessionId: string) => void;
};

/// The docked right column on wide windows, and only on the views that have
/// a list to inspect. With an event selected it is the event inspector; on
/// Timeline it falls back to the open session; otherwise it waits quietly.
export function InspectorPane(
  props: Omit<EventInspectorProps, "event"> & { event: AgentEvent | null; context: InspectorContext },
) {
  const { context } = props;
  const sessionOpen = context.view === "timeline" && context.session !== null;
  return (
    <aside className="inspector" aria-label="Inspector">
      {props.event ? (
        <EventInspector {...props} event={props.event} />
      ) : sessionOpen && context.session ? (
        <SessionCard {...context} session={context.session} />
      ) : (
        <EmptyInspector />
      )}
    </aside>
  );
}

function EmptyInspector() {
  return (
    <div className="insp-empty">
      <img className="insp-empty-mascot" src="/mascot/inspector.png" alt="" />
      <p>Select a row to inspect it</p>
      <ul className="insp-keys">
        <li>
          <kbd>↑</kbd>
          <kbd>↓</kbd> move
        </li>
        <li>
          <kbd>A</kbd> acknowledge
        </li>
        <li>
          <kbd>⌘K</kbd> search
        </li>
      </ul>
    </div>
  );
}

function SessionCard(props: InspectorContext & { session: SessionSummary }) {
  const s = props.session;
  const flaggedHere = props.sessionEvents.filter((e) => e.flag);
  const hasGap = s.hook_tool_count > 0 && s.tail_tool_count > 0;
  return (
    <>
      <header className="insp-head">
        <div className="insp-title">
          <h2>{projectName(s.cwd)}</h2>
          <p>
            <span className={`agent-chip agent-${s.agent}`}>{agentLabel(s.agent)}</span> · started{" "}
            {relTime(s.started_at)} · {durationLabel(s.started_at, s.last_at)}
          </p>
        </div>
      </header>

      {s.first_prompt && <p className="insp-prompt">{s.first_prompt}</p>}

      <dl className="insp-stats">
        <div>
          <dd>{s.event_count}</dd>
          <dt>events</dt>
        </div>
        <div>
          <dd>{s.command_count}</dd>
          <dt>commands</dt>
        </div>
        <div className={s.flagged_count > 0 ? "bad" : ""}>
          <dd>{s.flagged_count}</dd>
          <dt>flagged</dt>
        </div>
      </dl>

      {hasGap && (
        <p
          className="insp-note"
          title="Hooks were off for part of this session, or Tracon was not running. Those tool calls were recovered from the transcript."
        >
          {s.tail_tool_count} tool calls recovered from the transcript (hooks were off)
        </p>
      )}

      <div className="insp-actions">
        {hasTranscript(s.agent) && (
          <button className="btn-dark" onClick={() => props.onReadSession(s)}>
            Conversation
          </button>
        )}
        <button className="ack-btn" onClick={props.onExportSession}>
          Export JSON
        </button>
        <ConfirmButton
          label="Delete session"
          confirmLabel={`Really delete ${s.event_count} events?`}
          onConfirm={() => props.onDeleteSession(s.session_id)}
        />
      </div>

      {flaggedHere.length > 0 && (
        <section className="insp-section">
          <h3>Flagged in this session</h3>
          <ul className="insp-list">
            {flaggedHere.slice(0, 8).map((e, i) => (
              <li key={e.id ?? i}>
                <button onClick={() => props.onOpenEvent(e)}>
                  <span className="flag-chip">{e.flag}</span>
                  <span className="mono">{e.summary}</span>
                </button>
              </li>
            ))}
          </ul>
        </section>
      )}
    </>
  );
}
