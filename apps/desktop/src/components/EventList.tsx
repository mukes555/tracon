import { useEffect, useMemo, useState } from "react";
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
  /// Called with the rows actually rendered, so keyboard stepping stays on screen.
  onVisibleRows: (events: AgentEvent[]) => void;
}) {
  const [limit, setLimit] = useState(INITIAL_ROWS);

  // Events arrive oldest-first, so the recent tail is the end of the list.
  const shown = useMemo(() => props.events.slice(-limit), [props.events, limit]);
  const hidden = props.events.length - shown.length;
  const { onVisibleRows } = props;
  useEffect(() => {
    onVisibleRows(shown);
    return () => onVisibleRows([]);
  }, [shown, onVisibleRows]);

  return (
    <ul className="rows" role="listbox" aria-label="Session events">
      {hidden > 0 && (
        <li className="list-more" role="none">
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
