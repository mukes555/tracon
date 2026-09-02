import { useState } from "react";
import type { AgentEvent, CaptureStatus, DayCount, LiveSession, Stats, View } from "../lib/types";
import { agentLabel, projectName, relTime, timeOf } from "../lib/format";

// Persisted UI preference: the live board's expanded detail mode. A missing
// or blocked localStorage silently falls back to the compact view.
const LIVE_DETAILS_KEY = "tracon-live-details";

export function OverviewView(props: {
  stats: Stats | null;
  days: DayCount[];
  capture: CaptureStatus | null;
  recentFlagged: AgentEvent[];
  recentPackages: AgentEvent[];
  liveSessions: LiveSession[];
  onNavigate: (v: View) => void;
  onOpenEvent: (event: AgentEvent) => void;
  onAck: (event: AgentEvent) => void;
  onOpenSession: (sessionId: string) => void;
}) {
  const { stats } = props;
  const [liveDetails, setLiveDetails] = useState(() => {
    try {
      return localStorage.getItem(LIVE_DETAILS_KEY) === "true";
    } catch {
      return false;
    }
  });
  const toggleLiveDetails = () => {
    const next = !liveDetails;
    setLiveDetails(next);
    try {
      localStorage.setItem(LIVE_DETAILS_KEY, String(next));
    } catch {
      // preference just will not persist
    }
  };
  return (
    <main className="view overview">
      <header className="view-head">
        <h1>Overview</h1>
        <p className="view-sub">What your AI agents did on this machine.</p>
      </header>

      {props.liveSessions.length > 0 && (
        <section className="card live-card">
          <div className="card-head">
            <h3>Live now</h3>
            <label className="live-toggle">
              <input
                type="checkbox"
                className="switch-input"
                checked={liveDetails}
                onChange={toggleLiveDetails}
              />
              <span className="switch" aria-hidden="true" />
              details
            </label>
          </div>
          <ul className="live-list">
            {props.liveSessions.map((s) => (
              <li key={s.session_id}>
                <button className="live-row" onClick={() => props.onOpenSession(s.session_id)}>
                  <span className="pulse-dot" />
                  <span className="live-main">
                    <span className="live-head">
                      <span className="live-agent">{agentLabel(s.agent)}</span>
                      <span className="live-project">{projectName(s.cwd)}</span>
                      {s.flagged_count > 0 && (
                        <span className="flag-chip">{s.flagged_count} flagged</span>
                      )}
                      {s.subagent_count > 0 && (
                        <span className="live-sub">
                          {s.subagent_count} {s.subagent_count === 1 ? "subagent" : "subagents"}
                        </span>
                      )}
                      <span className="live-meta">active {relTime(s.last_ts)}</span>
                    </span>
                    {s.last_prompt && <span className="live-prompt">{s.last_prompt}</span>}
                    {liveDetails && s.subagents.length > 0 && (
                      <span className="live-subagents">running: {s.subagents.join(" · ")}</span>
                    )}
                    {liveDetails && s.last_action && (
                      <span className="live-action">{s.last_action}</span>
                    )}
                    {liveDetails && (
                      <span className="live-detail-meta">
                        {s.event_count} events in the live window · session{" "}
                        {s.session_id.slice(0, 8)}
                      </span>
                    )}
                  </span>
                </button>
              </li>
            ))}
          </ul>
        </section>
      )}

      <section className="card stat-strip">
        <Stat
          label="Sessions today"
          value={stats?.sessions_today}
          onOpen={() => props.onNavigate("timeline")}
        />
        <Stat
          label="Commands today"
          value={stats?.commands_today}
          onOpen={() => props.onNavigate("timeline")}
        />
        <Stat
          label="Packages today"
          value={stats?.packages_today}
          onOpen={() => props.onNavigate("packages")}
        />
        <Stat
          label="Flagged all time"
          value={stats?.flagged_count}
          tone={stats && stats.flagged_count > 0 ? "bad" : undefined}
          onOpen={() => props.onNavigate("flagged")}
        />
      </section>

      <section className="card">
        <h3>Activity · last 14 days</h3>
        <ActivityBars days={props.days} />
      </section>

      <div className="two-col">
        <section className="card">
          <div className="card-head">
            <h3>Flag inbox</h3>
            <button className="linkish" onClick={() => props.onNavigate("flagged")}>
              view all
            </button>
          </div>
          {props.recentFlagged.length === 0 ? (
            <p className="muted">Nothing flagged. Quiet skies.</p>
          ) : (
            <ul className="flag-inbox">
              {props.recentFlagged.slice(0, 5).map((e, i) => (
                <li key={e.id ?? i} className="flag-inbox-row">
                  <button className="flag-inbox-body" onClick={() => props.onOpenEvent(e)}>
                    <span className="flag-chip">{e.flag}</span>
                    <span className="flag-inbox-cmd">{e.summary}</span>
                    <span className="flag-inbox-meta">
                      {projectName(e.cwd)} · {timeOf(e.ts)}
                    </span>
                  </button>
                  <button className="ack-btn" onClick={() => props.onAck(e)}>
                    Ack
                  </button>
                </li>
              ))}
            </ul>
          )}
        </section>

        <section className="card">
          <div className="card-head">
            <h3>Recent packages</h3>
            <button className="linkish" onClick={() => props.onNavigate("packages")}>
              view all
            </button>
          </div>
          {props.recentPackages.length === 0 ? (
            <p className="muted">No package installs recorded yet.</p>
          ) : (
            <MiniList
              events={props.recentPackages.slice(0, 5)}
              onOpen={props.onOpenEvent}
            />
          )}
        </section>
      </div>

      <CaptureCard capture={props.capture} />
    </main>
  );
}

function Stat(props: {
  label: string;
  value?: number | null;
  tone?: "bad";
  onOpen: () => void;
}) {
  return (
    <button
      className={`stat-cell${props.tone ? " tone-bad" : ""}`}
      onClick={props.onOpen}
    >
      <span className="stat-value">{props.value?.toLocaleString() ?? "-"}</span>
      <span className="stat-label">{props.label}</span>
    </button>
  );
}

function ActivityBars(props: { days: DayCount[] }) {
  const days = fillMissingDays(props.days, 14);
  const max = Math.max(1, ...days.map((d) => d.events));
  return (
    <div className="bars" role="img" aria-label="Events per day, last 14 days">
      {days.map((d) => (
        <div key={d.day} className="bar-col" title={`${d.day}: ${d.events} events, ${d.flagged} flagged`}>
          <div className="bar-stack" style={{ height: `${Math.max(2, (d.events / max) * 100)}%` }}>
            {d.flagged > 0 && (
              <div
                className="bar-flagged"
                style={{ height: `${Math.min(100, (d.flagged / Math.max(1, d.events)) * 100)}%` }}
              />
            )}
          </div>
          <span className="bar-day">{d.day.slice(8)}</span>
        </div>
      ))}
    </div>
  );
}

function fillMissingDays(days: DayCount[], count: number): DayCount[] {
  const byDay = new Map(days.map((d) => [d.day, d]));
  const out: DayCount[] = [];
  const now = new Date();
  for (let i = count - 1; i >= 0; i--) {
    const d = new Date(now.getTime() - i * 24 * 60 * 60 * 1000);
    const key = d.toISOString().slice(0, 10);
    out.push(byDay.get(key) ?? { day: key, events: 0, flagged: 0 });
  }
  return out;
}

function MiniList(props: {
  events: AgentEvent[];
  onOpen: (event: AgentEvent) => void;
}) {
  return (
    <ul className="mini-list">
      {props.events.map((e, i) => (
        <li key={e.id ?? i}>
          <button className="mini-row" onClick={() => props.onOpen(e)}>
            <span className="mini-time">{timeOf(e.ts)}</span>
            <span className="mini-body">
              <span className="mini-project">{projectName(e.cwd)}</span>
              <span className="mini-summary">{e.summary}</span>
            </span>
          </button>
        </li>
      ))}
    </ul>
  );
}

const CURSOR_HOOK_CMD =
  "sh -c 'cat | curl -s -m 5 -X POST -H \"content-type: application/json\" --data-binary @- http://localhost:48620/ingest -o /dev/null; exit 0'";

const SETUP_SOURCES: {
  key: string;
  label: string;
  agent: string;
  source: string;
  auto?: string;
  how?: string;
  snippet?: string;
}[] = [
  {
    key: "claude-hooks",
    label: "Claude Code hooks",
    agent: "claude-code",
    source: "hook",
    how: "Real-time capture. Run Claude Code with the Tracon plugin:",
    snippet: "claude --plugin-dir <tracon-repo>/integrations/claude-plugin",
  },
  {
    key: "claude-tail",
    label: "Claude Code transcripts",
    agent: "claude-code",
    source: "log_tail",
    auto: "automatic: read-only tailing of ~/.claude/projects",
  },
  {
    key: "codex",
    label: "Codex rollouts",
    agent: "codex",
    source: "log_tail",
    auto: "automatic when Codex CLI is installed (~/.codex/sessions)",
  },
  {
    key: "cursor",
    label: "Cursor hooks",
    agent: "cursor",
    source: "hook",
    how: "Merge this hook into ~/.cursor/hooks.json (events: beforeShellExecution, afterFileEdit, beforeSubmitPrompt...), then restart Cursor:",
    snippet: `{ "version": 1, "hooks": { "beforeShellExecution": [{ "command": "${CURSOR_HOOK_CMD}" }] } }`,
  },
  {
    key: "gemini",
    label: "Gemini hooks",
    agent: "gemini",
    source: "hook",
    how: "Add BeforeTool/AfterTool command hooks in ~/.gemini/settings.json posting to the /ingest/gemini endpoint. Full snippet: integrations/gemini-hooks/README.md",
    snippet:
      "http://localhost:48620/ingest/gemini",
  },
];

function CaptureCard(props: { capture: CaptureStatus | null }) {
  const [openKey, setOpenKey] = useState<string | null>(null);
  const [copiedKey, setCopiedKey] = useState<string | null>(null);
  if (!props.capture) return null;
  const { counts } = props.capture;
  const countOf = (agent: string, source: string) =>
    counts.find((c) => c.agent === agent && c.source === source)?.count ?? 0;

  const copy = async (key: string, text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopiedKey(key);
      setTimeout(() => setCopiedKey(null), 2500);
    } catch {
      // Clipboard unavailable; the snippet is still visible to copy by hand.
    }
  };

  return (
    <section className="card">
      <h3>Capture sources</h3>
      <ul className="setup-list">
        {SETUP_SOURCES.map((s) => {
          const live = countOf(s.agent, s.source) > 0;
          const expandable = !live && !s.auto;
          return (
            <li key={s.key}>
              <button
                className="setup-row"
                onClick={() => expandable && setOpenKey(openKey === s.key ? null : s.key)}
                disabled={!expandable}
              >
                <span className={live ? "dot ok" : "dot"} />
                <span className="capture-label">{s.label}</span>
                <span className="capture-detail">
                  {live
                    ? `live · ${countOf(s.agent, s.source)} events`
                    : (s.auto ?? "not connected · click to set up")}
                </span>
              </button>
              {openKey === s.key && s.how && (
                <div className="setup-detail">
                  <p className="muted">{s.how}</p>
                  {s.snippet && (
                    <div className="snippet-row">
                      <code className="snippet">{s.snippet}</code>
                      <button className="chip" onClick={() => copy(s.key, s.snippet ?? "")}>
                        {copiedKey === s.key ? "copied" : "copy"}
                      </button>
                    </div>
                  )}
                </div>
              )}
            </li>
          );
        })}
      </ul>
    </section>
  );
}
