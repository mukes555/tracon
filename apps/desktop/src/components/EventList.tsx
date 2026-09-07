import { useState } from "react";
import type { AgentEvent } from "../lib/types";
import { EventRow } from "./EventRow";

// Sessions can hold hundreds of rows; rendering them all at once is what
// makes a view feel heavy. Render the recent tail, reveal the rest on demand.
const INITIAL_ROWS = 150;

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
      {shown.map((e, i) => (
        <EventRow
          key={e.id ?? i}
          event={e}
          selected={e.id !== undefined && e.id === props.selectedId}
          showProject={props.showProject}
          advanced={props.advanced}
          onOpen={props.onOpen}
        />
      ))}
    </ul>
  );
}
