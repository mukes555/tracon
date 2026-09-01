import { useEffect, useMemo, useRef, useState } from "react";
import { api } from "../lib/api";
import type { AgentEvent, SessionSummary, View } from "../lib/types";
import { agentLabel, projectName, timeOf } from "../lib/format";
import { kindIcon } from "./icons";

type Item =
  | { kind: "nav"; label: string; view: View }
  | { kind: "session"; label: string; meta: string; sessionId: string }
  | { kind: "event"; event: AgentEvent };

const NAV_ITEMS: { label: string; view: View }[] = [
  { label: "Go to Overview", view: "overview" },
  { label: "Go to Timeline", view: "timeline" },
  { label: "Go to Packages", view: "packages" },
  { label: "Go to Flagged", view: "flagged" },
  { label: "Go to Settings", view: "settings" },
];

export function CommandPalette(props: {
  sessions: SessionSummary[];
  onNavigate: (v: View) => void;
  onOpenSession: (sessionId: string) => void;
  onOpenEvent: (event: AgentEvent) => void;
  onClose: () => void;
}) {
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<AgentEvent[]>([]);
  const [active, setActive] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  // Debounced global search across all recorded history.
  useEffect(() => {
    if (query.trim().length < 2) {
      setHits([]);
      return;
    }
    const timer = setTimeout(() => {
      api.searchEvents(query).then(setHits).catch(() => setHits([]));
    }, 180);
    return () => clearTimeout(timer);
  }, [query]);

  const items = useMemo<Item[]>(() => {
    const q = query.toLowerCase();
    const nav: Item[] = NAV_ITEMS.filter((n) => !q || n.label.toLowerCase().includes(q)).map(
      (n) => ({ kind: "nav", ...n }),
    );
    const sessions: Item[] = props.sessions
      .filter(
        (s) =>
          q &&
          (projectName(s.cwd).toLowerCase().includes(q) ||
            (s.first_prompt ?? "").toLowerCase().includes(q)),
      )
      .slice(0, 5)
      .map((s) => ({
        kind: "session",
        label: projectName(s.cwd),
        meta: `${agentLabel(s.agent)} · ${s.event_count} events · ${timeOf(s.last_at)}`,
        sessionId: s.session_id,
      }));
    const events: Item[] = hits.slice(0, 30).map((event) => ({ kind: "event", event }));
    return [...sessions, ...events, ...nav];
  }, [query, hits, props.sessions]);

  useEffect(() => {
    setActive(0);
  }, [query, hits.length]);

  const choose = (item: Item) => {
    if (item.kind === "nav") props.onNavigate(item.view);
    if (item.kind === "session") props.onOpenSession(item.sessionId);
    // A found event opens directly in the slide-over; jumping to the
    // session timeline would land on the recent tail, not the hit.
    if (item.kind === "event") props.onOpenEvent(item.event);
    props.onClose();
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") props.onClose();
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setActive((a) => Math.min(a + 1, items.length - 1));
    }
    if (e.key === "ArrowUp") {
      e.preventDefault();
      setActive((a) => Math.max(a - 1, 0));
    }
    if (e.key === "Enter" && items[active]) {
      e.preventDefault();
      choose(items[active]);
    }
  };

  return (
    <div className="palette-backdrop" onClick={props.onClose}>
      <div
        className="palette"
        role="dialog"
        aria-label="Command palette"
        onClick={(e) => e.stopPropagation()}
      >
        <input
          ref={inputRef}
          className="palette-input"
          placeholder="Search everything your agents did..."
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={onKeyDown}
        />
        <ul className="palette-list">
          {items.length === 0 && (
            <li className="palette-empty">
              {query.trim().length < 2 ? "Type to search all history" : "No matches"}
            </li>
          )}
          {items.map((item, i) => (
            <li key={i}>
              <button
                className={i === active ? "palette-item active" : "palette-item"}
                onMouseEnter={() => setActive(i)}
                onClick={() => choose(item)}
              >
                {item.kind === "nav" && (
                  <>
                    <span className="palette-kind">›</span>
                    <span className="palette-label">{item.label}</span>
                  </>
                )}
                {item.kind === "session" && (
                  <>
                    <span className="palette-kind">◈</span>
                    <span className="palette-label">{item.label}</span>
                    <span className="palette-meta">{item.meta}</span>
                  </>
                )}
                {item.kind === "event" && (
                  <>
                    <span className="palette-kind">
                      {kindIcon(item.event.kind, item.event.tool_name)}
                    </span>
                    <span className="palette-label mono">{item.event.summary}</span>
                    <span className="palette-meta">
                      {projectName(item.event.cwd)} · {timeOf(item.event.ts)}
                    </span>
                  </>
                )}
              </button>
            </li>
          ))}
        </ul>
        <div className="palette-foot">
          <kbd className="kbd">↑↓</kbd> navigate
          <kbd className="kbd">↵</kbd> open
          <kbd className="kbd">esc</kbd> close
        </div>
      </div>
    </div>
  );
}
