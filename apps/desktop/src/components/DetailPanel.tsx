import { useEffect, useState } from "react";
import { api } from "../lib/api";
import type { AgentEvent } from "../lib/types";
import { agentLabel, projectName, timeOf } from "../lib/format";
import { kindIcon } from "./icons";

/// Right slide-over showing one event in full. Replaces the inline row
/// expanders everywhere: meta, raw payload, and the actions (acknowledge,
/// read thread) live here, so lists stay lists and context is never lost.
export function DetailPanel(props: {
  event: AgentEvent;
  acked?: boolean;
  onClose: () => void;
  onReadThread: (event: AgentEvent) => void;
  onAck: (event: AgentEvent, acked: boolean) => void;
  onOpenSession: (sessionId: string) => void;
}) {
  const e = props.event;
  const [payload, setPayload] = useState<unknown>(undefined);

  useEffect(() => {
    setPayload(undefined);
    if (e.id === undefined) return;
    api
      .eventPayload(e.id)
      .then(setPayload)
      .catch(() => setPayload(null));
  }, [e.id]);

  useEffect(() => {
    const onKey = (ev: KeyboardEvent) => {
      if (ev.key === "Escape") props.onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [props.onClose]);

  return (
    <div className="drawer-backdrop" onClick={props.onClose}>
      <aside className="drawer" role="dialog" aria-label="Event detail" onClick={(ev) => ev.stopPropagation()}>
        <header className="drawer-head">
          <span className="drawer-kind">{kindIcon(e.kind, e.tool_name)}</span>
          <div className="drawer-title">
            <h2>{e.tool_name ?? e.kind}</h2>
            <p>
              {projectName(e.cwd)} · <span className={`agent-chip agent-${e.agent}`}>{agentLabel(e.agent)}</span> · {timeOf(e.ts)}
            </p>
          </div>
          <button className="thread-close" onClick={props.onClose} aria-label="Close">
            ✕
          </button>
        </header>

        {e.flag && <p className="drawer-flag">{e.flag}</p>}

        {e.summary && <code className="drawer-summary">{e.summary}</code>}

        <div className="drawer-actions">
          <button className="btn-dark" onClick={() => props.onReadThread(e)}>
            Read thread
          </button>
          <button className="ack-btn" onClick={() => props.onOpenSession(e.session_id)}>
            View in timeline
          </button>
          {e.flag && (
            <button className="ack-btn" onClick={() => props.onAck(e, !(props.acked ?? false))}>
              {props.acked ? "Reopen" : "Acknowledge"}
            </button>
          )}
        </div>

        <div className="drawer-meta">
          <span>session {e.session_id.slice(0, 8)}</span>
          <span>source: {e.source}</span>
          <span>kind: {e.kind}</span>
          {e.cwd && <span>{e.cwd}</span>}
        </div>

        <pre className="drawer-payload">
          {payload === undefined ? "loading..." : JSON.stringify(payload, null, 2)}
        </pre>
      </aside>
    </div>
  );
}
