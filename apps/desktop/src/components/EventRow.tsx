import { useEffect, useRef } from "react";
import type { AgentEvent } from "../lib/types";
import { severityOf } from "../lib/flags";
import { agentLabel, kindLabel, projectName, timeOf } from "../lib/format";
import { packageParts } from "../lib/packages";
import { InfoIcon, TypeTile } from "./icons";

/// The one list row. The title is what happened (the command, the file, the
/// packages, the prompt), never the tool name; the subline says what kind of
/// thing it was, why it was flagged, and where. Every list in the app renders
/// through here so they all read the same.
export function EventRow(props: {
  event: AgentEvent;
  selected?: boolean;
  showProject?: boolean;
  advanced?: boolean;
  /// Rendered beside the row, outside the clickable area (e.g. acknowledge).
  action?: React.ReactNode;
  onOpen: (event: AgentEvent) => void;
}) {
  const e = props.event;
  // Keyboard stepping selects rows that may be off screen.
  const ref = useRef<HTMLLIElement>(null);
  useEffect(() => {
    if (props.selected) ref.current?.scrollIntoView({ block: "nearest" });
  }, [props.selected]);
  const classes = ["row"];
  if (e.flag) classes.push("flagged");
  if (props.selected) classes.push("selected");

  const row = (
    <button className={classes.join(" ")} onClick={() => props.onOpen(e)}>
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
      {!props.action && (
        <span className="row-action" aria-hidden="true">
          <InfoIcon />
        </span>
      )}
    </button>
  );

  if (!props.action) return <li ref={ref}>{row}</li>;
  return (
    <li ref={ref} className="row-line">
      {row}
      {props.action}
    </li>
  );
}

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
