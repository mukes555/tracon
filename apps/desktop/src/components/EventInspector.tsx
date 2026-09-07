import { useEffect, useMemo, useState } from "react";
import { api } from "../lib/api";
import { flagWhy, severityOf } from "../lib/flags";
import type { AgentEvent } from "../lib/types";
import { agentLabel, hasTranscript, kindLabel, projectName, timeOf } from "../lib/format";
import { useFocusTrap } from "../lib/useFocusTrap";
import { ChevronIcon, TypeTile } from "./icons";

export type EventInspectorProps = {
  event: AgentEvent;
  acked?: boolean;
  advanced: boolean;
  /// Where the event sits in the list it was opened from, for prev/next.
  position: { index: number; total: number } | null;
  onStep: (delta: 1 | -1) => void;
  onClose: () => void;
  onReadThread: (event: AgentEvent) => void;
  onAck: (event: AgentEvent, acked: boolean) => void;
  onOpenSession: (sessionId: string) => void;
};

/// One event in full: what ran, why it was flagged, what to do about it,
/// and the raw payload folded away for when the summary is not enough.
export function EventInspector(props: EventInspectorProps) {
  const e = props.event;
  const [payload, setPayload] = useState<unknown>(undefined);

  // Stepping quickly through rows fires one fetch per step; a slow reply
  // for an earlier row must not overwrite the current one.
  useEffect(() => {
    setPayload(undefined);
    if (e.id === undefined) return;
    let stale = false;
    api
      .eventPayload(e.id)
      .then((p) => {
        if (!stale) setPayload(p);
      })
      .catch(() => {
        if (!stale) setPayload(null);
      });
    return () => {
      stale = true;
    };
  }, [e.id]);
  const payloadText = useMemo(
    () => (payload === undefined ? "loading..." : JSON.stringify(payload, null, 2)),
    [payload],
  );

  const pos = props.position;
  const hasPrev = pos !== null && pos.index > 0;
  const hasNext = pos !== null && pos.index < pos.total - 1;
  const severity = e.flag ? severityOf(e.flag, e.summary) : null;

  return (
    <>
      <div className="insp-toolbar">
        {pos && (
          <>
            <button className="icon-btn" disabled={!hasPrev} onClick={() => props.onStep(-1)} aria-label="Previous event" title="Previous (↑)">
              <ChevronIcon dir="up" />
            </button>
            <button className="icon-btn" disabled={!hasNext} onClick={() => props.onStep(1)} aria-label="Next event" title="Next (↓)">
              <ChevronIcon dir="down" />
            </button>
            <span className="insp-pos">{pos.index + 1} of {pos.total}</span>
          </>
        )}
        <button className="icon-btn insp-close" onClick={props.onClose} aria-label="Close" title="Close (Esc)">
          ✕
        </button>
      </div>
      <header className="insp-head">
        <TypeTile kind={e.kind} toolName={e.tool_name} flagged={!!e.flag} />
        <div className="insp-title">
          <h2>{kindLabel(e)}</h2>
          <p>
            {projectName(e.cwd)} · <span className={`agent-chip agent-${e.agent}`}>{agentLabel(e.agent)}</span> · {timeOf(e.ts)}
          </p>
        </div>
      </header>

      {e.summary && (
        <code className={e.kind === "prompt" ? "insp-block prose" : "insp-block"}>{e.summary}</code>
      )}

      {e.flag && (
        <section className={`insp-why sev-${severity}`}>
          <span className={`flag-chip sev-${severity}`}>{e.flag}</span>
          <p>{flagWhy(e.flag)}</p>
        </section>
      )}

      <div className="insp-actions">
        {e.flag && (
          <button className="btn-dark" onClick={() => props.onAck(e, !(props.acked ?? false))}>
            {props.acked ? "Reopen" : "Acknowledge"}
          </button>
        )}
        {hasTranscript(e.agent) && (
          <button className={e.flag ? "ack-btn" : "btn-dark"} onClick={() => props.onReadThread(e)}>
            Read thread
          </button>
        )}
        <button className="ack-btn" onClick={() => props.onOpenSession(e.session_id)}>
          View in timeline
        </button>
      </div>

      <dl className="insp-meta">
        <dt>Session</dt>
        <dd>{e.session_id.slice(0, 8)}</dd>
        <dt>Source</dt>
        <dd>{e.source}</dd>
        {e.tool_name && (
          <>
            <dt>Tool</dt>
            <dd>{e.tool_name}</dd>
          </>
        )}
        {e.cwd && (
          <>
            <dt>Directory</dt>
            <dd className="mono">{e.cwd}</dd>
          </>
        )}
      </dl>

      <details className="insp-raw" open={props.advanced}>
        <summary>Raw payload</summary>
        <pre>{payloadText}</pre>
      </details>
    </>
  );
}

/// Narrow windows: the same inspector as a slide-over with a backdrop.
/// Escape is handled once, app-wide; focus is trapped here.
export function DetailPanel(props: EventInspectorProps) {
  const drawerRef = useFocusTrap<HTMLElement>();
  return (
    <div className="drawer-backdrop" onClick={props.onClose}>
      <aside
        ref={drawerRef}
        className="drawer"
        role="dialog"
        aria-modal="true"
        aria-label="Event detail"
        tabIndex={-1}
        onClick={(ev) => ev.stopPropagation()}
      >
        <EventInspector {...props} />
      </aside>
    </div>
  );
}
