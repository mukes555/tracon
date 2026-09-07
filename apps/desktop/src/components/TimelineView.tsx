import type { AgentEvent, KindFilter, SessionSummary } from "../lib/types";
import { agentLabel, durationLabel, hasTranscript, projectName, relTime } from "../lib/format";
import { ConfirmButton } from "./ConfirmButton";
import { EventList } from "./EventList";
import { FilterBar } from "./FilterBar";
import { SessionList } from "./SessionList";
import { KeyHints, StatusBar } from "./StatusBar";

/// Sessions on the left, the selected session's events on the right.
export function TimelineView(props: {
  sessions: SessionSummary[];
  selected: string | null;
  session: SessionSummary | null;
  live: boolean;
  events: AgentEvent[];
  filteredEvents: AgentEvent[];
  query: string;
  kind: KindFilter;
  updatedAt: string | null;
  advanced: boolean;
  selectedId?: number;
  onSelect: (sessionId: string) => void;
  onQuery: (q: string) => void;
  onKind: (k: KindFilter) => void;
  onExport: () => void;
  onDelete: (sessionId: string) => void;
  onReadThread: (session: SessionSummary) => void;
  onOpen: (event: AgentEvent) => void;
  onVisibleRows: (events: AgentEvent[]) => void;
}) {
  const s = props.session;
  return (
    <div className="body">
      <aside className="sessions">
        <SessionList sessions={props.sessions} selected={props.selected} onSelect={props.onSelect} />
      </aside>
      <main className="timeline">
        {s ? (
          <>
            <div className="timeline-toolbar">
              <SessionHeader
                session={s}
                live={props.live}
                onExport={props.onExport}
                onDelete={() => props.onDelete(s.session_id)}
                onReadThread={() => props.onReadThread(s)}
              />
              <FilterBar query={props.query} onQuery={props.onQuery} kind={props.kind} onKind={props.onKind} />
            </div>
            {/* Key by session so the show-more window resets when the user
                switches sessions. */}
            <EventList
              key={s.session_id}
              events={props.filteredEvents}
              showProject={false}
              advanced={props.advanced}
              selectedId={props.selectedId}
              onOpen={props.onOpen}
              onVisibleRows={props.onVisibleRows}
            />
            <StatusBar
              left={`${props.filteredEvents.length} of ${props.events.length} events · updated ${props.updatedAt ? relTime(props.updatedAt) : "..."}`}
              right={<KeyHints ack />}
            />
          </>
        ) : (
          <div className="pkg-empty">
            <p>Select a session to see its timeline.</p>
            <p className="muted">Every command, file edit, and install, in the order it happened.</p>
          </div>
        )}
      </main>
    </div>
  );
}

function SessionHeader(props: {
  session: SessionSummary;
  live: boolean;
  onExport: () => void;
  onDelete: () => void;
  onReadThread: () => void;
}) {
  const s = props.session;
  const hasGap = s.hook_tool_count > 0 && s.tail_tool_count > 0;
  return (
    <div className="session-header">
      <div>
        <h1 className="session-title">
          {props.live && <span className="pulse-dot" title="Active in the last 5 minutes" />}
          {projectName(s.cwd)}
        </h1>
        <p className="view-sub">
          {agentLabel(s.agent)} · {s.event_count} events · {s.command_count} commands ·{" "}
          {durationLabel(s.started_at, s.last_at)}
          {hasGap && (
            <span
              className="gap-chip"
              title="Hooks were off for part of this session, or Tracon was not running. Those tool calls were recovered from the transcript."
            >
              {s.tail_tool_count} recovered from transcript (hooks were off)
            </span>
          )}
        </p>
      </div>
      <div className="session-actions">
        {hasTranscript(s.agent) && (
          <button className="btn-dark" onClick={props.onReadThread}>
            Conversation
          </button>
        )}
        <button className="ack-btn" onClick={props.onExport}>
          Export JSON
        </button>
        <ConfirmButton
          label="Delete session"
          confirmLabel={`Really delete ${s.event_count} events?`}
          onConfirm={props.onDelete}
        />
      </div>
    </div>
  );
}
