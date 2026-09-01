import { useState } from "react";
import type { AgentEvent } from "../lib/types";
import { projectName, timeOf } from "../lib/format";
import { kindIcon } from "./icons";

// Sessions can hold hundreds of rows; rendering them all at once is what
// makes a view feel heavy. Render the recent tail, reveal the rest on demand.
const INITIAL_ROWS = 150;

export function EventList(props: {
  events: AgentEvent[];
  showProject: boolean;
  onOpen: (event: AgentEvent) => void;
}) {
  const [limit, setLimit] = useState(INITIAL_ROWS);

  // Events arrive oldest-first, so the recent tail is the end of the list.
  const shown = props.events.slice(-limit);
  const hidden = props.events.length - shown.length;

  return (
    <ul className="events">
      {hidden > 0 && (
        <li className="list-more">
          <button className="thread-btn" onClick={() => setLimit(Number.POSITIVE_INFINITY)}>
            Show {hidden} earlier events
          </button>
        </li>
      )}
      {shown.map((e, i) => (
        <li key={e.id ?? i}>
          <button
            className={`event kind-${e.kind}${e.flag ? " flagged" : ""}`}
            onClick={() => props.onOpen(e)}
          >
            <span className="event-time">{timeOf(e.ts)}</span>
            <span className="event-kind">
              <span className="event-icon">{kindIcon(e.kind, e.tool_name)}</span>
              {e.tool_name ?? e.kind}
            </span>
            <span className="event-summary">
              {props.showProject && (
                <span className="event-project">{projectName(e.cwd)} · </span>
              )}
              {e.summary ?? ""}
              {e.flag && <span className="flag-chip">{e.flag}</span>}
            </span>
          </button>
        </li>
      ))}
    </ul>
  );
}
