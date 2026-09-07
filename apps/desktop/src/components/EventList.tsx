import { useState } from "react";
import type { AgentEvent } from "../lib/types";
import { projectName, timeOf } from "../lib/format";
import { InfoIcon, TypeTile } from "./icons";

// Sessions can hold hundreds of rows; rendering them all at once is what
// makes a view feel heavy. Render the recent tail, reveal the rest on demand.
const INITIAL_ROWS = 150;

/** Class list for a list row: flagged tint, plus selected when it is the
    event open in the inspector. */
export function rowClass(event: AgentEvent, selectedId: number | undefined): string {
  const parts = ["row"];
  if (event.flag) parts.push("flagged");
  if (event.id !== undefined && event.id === selectedId) parts.push("selected");
  return parts.join(" ");
}

export function EventList(props: {
  events: AgentEvent[];
  showProject: boolean;
  advanced: boolean;
  selectedId?: number;
  onOpen: (event: AgentEvent) => void;
}) {
  const [limit, setLimit] = useState(INITIAL_ROWS);

  // Events arrive oldest-first, so the recent tail is the end of the list.
  const shown = props.events.slice(-limit);
  const hidden = props.events.length - shown.length;

  return (
    <ul className="rows">
      {hidden > 0 && (
        <li className="list-more">
          <button className="thread-btn" onClick={() => setLimit(Number.POSITIVE_INFINITY)}>
            Show {hidden} earlier events
          </button>
        </li>
      )}
      {shown.map((e, i) => {
        const isPrompt = e.kind === "prompt";
        return (
          <li key={e.id ?? i}>
            <button className={rowClass(e, props.selectedId)} onClick={() => props.onOpen(e)}>
              <TypeTile kind={e.kind} toolName={e.tool_name} flagged={!!e.flag} />
              <span className="row-main">
                <span className="row-title">
                  {isPrompt ? "Prompt" : (e.tool_name ?? e.kind)}
                  {e.flag && <span className="flag-chip">{e.flag}</span>}
                  {props.advanced && <span className="src-chip">{e.source}</span>}
                </span>
                <span className={isPrompt ? "row-sub prose" : "row-sub"}>
                  {props.showProject && <span className="row-project">{projectName(e.cwd)} · </span>}
                  {e.summary ?? ""}
                </span>
              </span>
              <span className="row-meta">{timeOf(e.ts)}</span>
              <span className="row-action" aria-hidden="true">
                <InfoIcon />
              </span>
            </button>
          </li>
        );
      })}
    </ul>
  );
}
