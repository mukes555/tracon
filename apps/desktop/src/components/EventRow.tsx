import { memo, useEffect, useRef } from "react";
import type { AgentEvent } from "../lib/types";
import { severityOf } from "../lib/flags";
import { agentLabel, kindLabel, projectName, timeOf } from "../lib/format";
import { packageParts } from "../lib/packages";
import { CheckIcon, InfoIcon, TypeTile } from "./icons";

/// The one list row. The title is what happened (the command, the file, the
/// packages, the prompt), never the tool name; the subline says what kind of
/// thing it was, why it was flagged, and where. Every list in the app renders
/// through here so they all read the same. Memoized: lists re-render on every
/// poll, and only the row whose selection changed needs to paint.
export const EventRow = memo(function EventRow(props: {
  event: AgentEvent;
  selected?: boolean;
  showProject?: boolean;
  advanced?: boolean;
  /// With onAck, an acknowledge (or reopen, when acked) button sits beside
  /// the row, outside the clickable area.
  acked?: boolean;
  onAck?: (event: AgentEvent, acked: boolean) => void;
  onOpen: (event: AgentEvent) => void;
}) {
  const e = props.event;
  const ref = useRef<HTMLButtonElement>(null);
  // Keyboard stepping selects rows that may be off screen, and moving focus
  // with the selection is what lets a screen reader announce the row. Focus
  // stays put while a dialog (slide-over, conversation) owns it.
  useEffect(() => {
    if (!props.selected) return;
    ref.current?.scrollIntoView({ block: "nearest" });
    const focusInDialog = Boolean(document.activeElement?.closest('[role="dialog"]'));
    if (!focusInDialog) ref.current?.focus();
  }, [props.selected]);
  const classes = ["row"];
  if (e.flag) classes.push("flagged");
  if (props.selected) classes.push("selected");

  const row = (
    <button
      ref={ref}
      className={classes.join(" ")}
      role="option"
      aria-selected={props.selected ?? false}
      onClick={() => props.onOpen(e)}
    >
      <TypeTile kind={e.kind} toolName={e.tool_name} flagged={!!e.flag} />
      <span className="row-main">
        <RowTitle event={e} />
        <span className="row-sub">
          {e.flag ? (
            <span className={`flag-chip sev-${severityOf(e.flag, e.summary)}`}>{e.flag}</span>
          ) : (
            <span>{kindLabel(e)}</span>
          )}
          {props.showProject && (
            <span className="row-where">
              {projectName(e.cwd)} · {agentLabel(e.agent)}
            </span>
          )}
          {props.advanced && <span className="src-chip">{e.source}</span>}
        </span>
      </span>
      <span className="row-meta">{timeOf(e.ts)}</span>
      {!props.onAck && (
        <span className="row-action" aria-hidden="true">
          <InfoIcon />
        </span>
      )}
    </button>
  );

  const { onAck } = props;
  if (!onAck) return <li role="none">{row}</li>;
  const label = props.acked ? "Reopen" : "Acknowledge";
  return (
    <li role="none" className={e.flag ? "row-line flagged" : "row-line"}>
      {row}
      <button
        className={props.acked ? "row-action-btn" : "row-action-btn ack"}
        title={label}
        aria-label={label}
        onClick={() => onAck(e, !props.acked)}
      >
        <CheckIcon size={14} />
      </button>
    </li>
  );
});

function RowTitle(props: { event: AgentEvent }) {
  const e = props.event;
  if (e.kind === "prompt") {
    return <span className="row-title prose">{e.summary}</span>;
  }
  if (e.kind === "package_install") {
    const p = packageParts(e);
    return (
      <span className="row-title pkg-names">
        <span className={`pkg-badge fam-${p.family}`}>{p.manager}</span>
        {p.appName ? (
          <span className="pkg-chip app-chip">{p.appName}</span>
        ) : p.names.length > 0 ? (
          p.names.map((n) => (
            <span key={n} className="pkg-chip">
              {n}
            </span>
          ))
        ) : (
          <span className="pkg-lockfile">install from lockfile</span>
        )}
      </span>
    );
  }
  return <span className="row-title mono">{e.summary || e.tool_name || e.kind}</span>;
}
