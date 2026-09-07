import type { AgentEvent, CaptureStatus, SessionSummary, Stats, View } from "../lib/types";
import { CAPTURE_SOURCES, sourceEventCount } from "../lib/captureSources";
import { agentLabel, durationLabel, projectName, relTime } from "../lib/format";
import { EventInspector, type EventInspectorProps } from "./EventInspector";

type Context = {
  view: View;
  session: SessionSummary | null;
  sessionEvents: AgentEvent[];
  stats: Stats | null;
  capture: CaptureStatus | null;
  liveCount: number;
  updatedAt: string | null;
  onOpenEvent: (event: AgentEvent) => void;
  onNavigate: (view: View) => void;
  onReadSession: (session: SessionSummary) => void;
  onExportSession: () => void;
};

/// Wide windows: the docked right column. With an event selected it is the
/// event inspector; otherwise it carries the context of the current view,
/// the open session on Timeline or today's numbers everywhere else, so the
/// space is never blank.
export function InspectorPane(
  props: Omit<EventInspectorProps, "event"> & { event: AgentEvent | null; context: Context },
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
        <TodayCard {...context} />
      )}
    </aside>
  );
}

function SessionCard(props: Context & { session: SessionSummary }) {
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
        <button className="btn-dark" onClick={() => props.onReadSession(s)}>
          Conversation
        </button>
        <button className="ack-btn" onClick={props.onExportSession}>
          Export JSON
        </button>
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

      <p className="insp-hint">Select an event to inspect it.</p>
    </>
  );
}

function TodayCard(props: Context) {
  const st = props.stats;
  const n = (value: number | undefined) => value?.toLocaleString() ?? "-";
  return (
    <>
      <img className="inspector-mascot" src="/mascot/inspector.png" alt="" />
      <section className="insp-section">
        <h3>Today</h3>
        <dl className="insp-stats">
          <button onClick={() => props.onNavigate("timeline")}>
            <dd>{n(st?.sessions_today)}</dd>
            <dt>sessions</dt>
          </button>
          <button onClick={() => props.onNavigate("timeline")}>
            <dd>{n(st?.commands_today)}</dd>
            <dt>commands</dt>
          </button>
          <button onClick={() => props.onNavigate("packages")}>
            <dd>{n(st?.packages_today)}</dd>
            <dt>installs</dt>
          </button>
        </dl>
        <ul className="insp-kv">
          <li>
            <span>Agents live now</span>
            <b>{props.liveCount}</b>
          </li>
          <li className={st && st.flagged_count > 0 ? "bad" : ""}>
            <span>Open flags</span>
            <b>{n(st?.flagged_count)}</b>
          </li>
          <li>
            <span>Acknowledged</span>
            <b>{n(st?.acked_count)}</b>
          </li>
        </ul>
      </section>

      {props.capture && (
        <section className="insp-section">
          <h3>Capture</h3>
          <ul className="insp-kv">
            {CAPTURE_SOURCES.map((s) => {
              const live = sourceEventCount(props.capture, s) > 0;
              return (
                <li key={s.key}>
                  <span>
                    <span className={live ? "dot ok" : "dot"} /> {s.label}
                  </span>
                  <b className="muted">{live ? "live" : "off"}</b>
                </li>
              );
            })}
          </ul>
          <button className="linkish" onClick={() => props.onNavigate("overview")}>
            set up capture
          </button>
        </section>
      )}

      <p className="insp-hint">
        Select an event to inspect it.
        {props.updatedAt && <> Updated {relTime(props.updatedAt)}.</>}
      </p>
    </>
  );
}
