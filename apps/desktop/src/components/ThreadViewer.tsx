import { memo, useEffect, useMemo, useRef, useState } from "react";
import { api } from "../lib/api";
import type { ThreadMessage } from "../lib/types";
import { agentLabel, timeOf } from "../lib/format";

// Long sessions carry hundreds of large messages; rendering them all at once
// makes opening the viewer visibly lag. Show a window (the tail, or centered
// on the target) and reveal the rest only on request.
const WINDOW = 200;

// Where the viewer is anchored: the flagged/clicked message, the very first
// message, or the latest one. Start/End jumps re-anchor the window and scroll.
type Anchor = "target" | "start" | "end";

/// Read-only conversation view for one session, fetched on demand from the
/// agent's own transcript. Given a target ts (from a flagged event), the
/// message closest in time is highlighted and scrolled into view.
export const ThreadViewer = memo(function ThreadViewer(props: {
  sessionId: string;
  targetTs?: string;
  agent?: string;
  title?: string;
  onClose: () => void;
}) {
  const [messages, setMessages] = useState<ThreadMessage[] | null>(null);
  const [showAll, setShowAll] = useState(false);
  // tick makes a repeated click on the same jump button scroll again.
  const [jump, setJump] = useState<{ to: Anchor; tick: number }>({ to: "target", tick: 0 });
  const listRef = useRef<HTMLUListElement>(null);
  const targetRef = useRef<HTMLLIElement>(null);

  useEffect(() => {
    api
      .sessionThread(props.sessionId)
      .then(setMessages)
      .catch(() => setMessages([]));
  }, [props.sessionId]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") props.onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [props.onClose]);

  const targetIndex = useMemo(
    () => nearestMessageIndex(messages ?? [], props.targetTs),
    [messages, props.targetTs],
  );

  const visible = useMemo(
    () => windowFor(messages ?? [], targetIndex, jump.to, showAll),
    [messages, targetIndex, jump, showAll],
  );

  useEffect(() => {
    if (messages === null) return;
    if (jump.to === "start") {
      listRef.current?.scrollTo({ top: 0 });
      return;
    }
    if (jump.to === "end" || targetIndex === null) {
      listRef.current?.scrollTo({ top: listRef.current.scrollHeight });
      return;
    }
    targetRef.current?.scrollIntoView({ block: "center" });
  }, [messages, targetIndex, jump]);

  const jumpTo = (to: Anchor) => setJump((j) => ({ to, tick: j.tick + 1 }));

  return (
    <div className="palette-backdrop" onClick={props.onClose}>
      <div
        className="thread"
        role="dialog"
        aria-label="Conversation"
        onClick={(e) => e.stopPropagation()}
      >
        <header className="thread-head">
          <h2>{props.title ?? "Conversation"}</h2>
          {props.agent && (
            <span className={`agent-chip agent-${props.agent}`}>{agentLabel(props.agent)}</span>
          )}
          <span className="thread-session mono">{props.sessionId.slice(0, 8)}</span>
          <button className="thread-jump" onClick={() => jumpTo("start")}>
            ↑ Start
          </button>
          <button className="thread-jump" onClick={() => jumpTo("end")}>
            ↓ End
          </button>
          <button className="thread-close" onClick={props.onClose} aria-label="Close">
            ✕
          </button>
        </header>

        {messages === null && <p className="thread-note">Reading transcript...</p>}

        {messages !== null && messages.length === 0 && (
          <p className="thread-note">No conversation transcript available for this session.</p>
        )}

        {messages !== null && messages.length > 0 && (
          <ul className="thread-list" ref={listRef}>
            {visible.hidden > 0 && (
              <li className="thread-more">
                <button className="thread-btn" onClick={() => setShowAll(true)}>
                  Show all {messages.length} messages
                </button>
              </li>
            )}
            {visible.msgs.map((m, i) => {
              const index = visible.start + i;
              const isLast = index === messages.length - 1;
              const isTarget = targetIndex !== null ? index === targetIndex : isLast;
              return (
                <li
                  key={index}
                  ref={isTarget ? targetRef : undefined}
                  className={bubbleClass(m.role, targetIndex !== null && index === targetIndex)}
                >
                  <span className="thread-meta">
                    {m.role === "user" ? (
                      <span className="thread-who">You</span>
                    ) : (
                      <span className={`agent-chip agent-${props.agent ?? ""}`}>
                        {props.agent ? agentLabel(props.agent) : "Agent"}
                      </span>
                    )}
                    {m.ts && <span>{timeOf(m.ts)}</span>}
                  </span>
                  <p className="thread-text">{m.text}</p>
                </li>
              );
            })}
          </ul>
        )}

        <footer className="thread-foot">
          Read straight from the agent's transcript, read-only. Tool activity lives
          in the timeline.
        </footer>
      </div>
    </div>
  );
});

function windowFor(
  messages: ThreadMessage[],
  targetIndex: number | null,
  anchor: Anchor,
  showAll: boolean,
): { start: number; msgs: ThreadMessage[]; hidden: number } {
  if (showAll || messages.length <= WINDOW) {
    return { start: 0, msgs: messages, hidden: 0 };
  }
  const hidden = messages.length - WINDOW;
  const tailStart = messages.length - WINDOW;

  if (anchor === "start") {
    return { start: 0, msgs: messages.slice(0, WINDOW), hidden };
  }
  if (anchor === "end" || targetIndex === null) {
    return { start: tailStart, msgs: messages.slice(tailStart), hidden };
  }
  const start = Math.max(0, Math.min(tailStart, targetIndex - WINDOW / 2));
  return { start, msgs: messages.slice(start, start + WINDOW), hidden };
}

function bubbleClass(role: string, highlighted: boolean): string {
  const side = role === "user" ? "thread-msg user" : "thread-msg";
  return highlighted ? `${side} target` : side;
}

function nearestMessageIndex(messages: ThreadMessage[], targetTs?: string): number | null {
  if (!targetTs) return null;
  const target = new Date(targetTs).getTime();
  if (!Number.isFinite(target)) return null;

  let best: number | null = null;
  let bestDiff = Number.POSITIVE_INFINITY;
  messages.forEach((m, i) => {
    const t = new Date(m.ts).getTime();
    if (!Number.isFinite(t)) return;
    const diff = Math.abs(t - target);
    if (diff < bestDiff) {
      bestDiff = diff;
      best = i;
    }
  });
  return best;
}
