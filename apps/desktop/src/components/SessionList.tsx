import { useState } from "react";
import type { SessionSummary } from "../lib/types";
import { agentCounts, agentLabel, groupByDay, projectName, timeOf } from "../lib/format";
import { AgentChips } from "./AgentChips";

export function SessionList(props: {
  sessions: SessionSummary[];
  selected: string | null;
  onSelect: (sessionId: string) => void;
}) {
  const [query, setQuery] = useState("");
  const [agent, setAgent] = useState("all");

  if (props.sessions.length === 0) {
    return (
      <div className="empty">
        <p>No agent activity recorded yet.</p>
        <p>
          Run any Claude Code, Codex, Cursor, or Gemini session and it appears
          here. See Overview for capture setup.
        </p>
      </div>
    );
  }

  const q = query.toLowerCase();
  const visible = props.sessions.filter((s) => {
    if (agent !== "all" && s.agent !== agent) return false;
    if (!q) return true;
    return (
      projectName(s.cwd).toLowerCase().includes(q) ||
      (s.first_prompt ?? "").toLowerCase().includes(q) ||
      agentLabel(s.agent).toLowerCase().includes(q)
    );
  });

  const groups = groupByDay(visible, (s) => s.last_at);
  return (
    <>
      <input
        className="search sidebar-search"
        type="search"
        placeholder="Filter sessions..."
        value={query}
        onChange={(e) => setQuery(e.target.value)}
      />
      <AgentChips counts={agentCounts(props.sessions)} value={agent} onChange={setAgent} />
      {visible.length === 0 && <p className="muted">No sessions match.</p>}
      {groups.map((group) => (
        <div key={group.label} className="session-group">
          <h2>{group.label}</h2>
          <ul>
            {group.items.map((s) => (
              <li key={s.session_id}>
                <button
                  className={s.session_id === props.selected ? "session active" : "session"}
                  onClick={() => props.onSelect(s.session_id)}
                >
                  <span className="session-head">
                    <span className="session-project">{projectName(s.cwd)}</span>
                    <span className={`agent-chip agent-${s.agent}`}>{agentLabel(s.agent)}</span>
                    {s.flagged_count > 0 && (
                      <span className="session-flags">{s.flagged_count} flagged</span>
                    )}
                  </span>
                  {s.first_prompt && <span className="session-prompt">{s.first_prompt}</span>}
                  <span className="session-meta">
                    {s.event_count} events · {s.command_count} commands · {timeOf(s.last_at)}
                  </span>
                </button>
              </li>
            ))}
          </ul>
        </div>
      ))}
    </>
  );
}
