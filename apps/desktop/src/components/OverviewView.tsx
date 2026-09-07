import { useState } from "react";
import type { AgentEvent, CaptureStatus, DayCount, LiveSession, Stats, View } from "../lib/types";
import { agentLabel, projectName, relTime } from "../lib/format";
import { CAPTURE_SOURCES, sourceEventCount } from "../lib/captureSources";
import { EventRow } from "./EventRow";
import { CheckIcon } from "./icons";

export function OverviewView(props: {
  stats: Stats | null;
  days: DayCount[];
  capture: CaptureStatus | null;
  recentFlagged: AgentEvent[];
  recentPackages: AgentEvent[];
  liveSessions: LiveSession[];
  advanced: boolean;
  onNavigate: (v: View) => void;
  onOpenEvent: (event: AgentEvent) => void;
  onAck: (event: AgentEvent) => void;
  onOpenSession: (sessionId: string) => void;
}) {
  const { stats } = props;
  const liveDetails = props.advanced;
  return (
    <main className="view overview">
      <header className="view-head">
        <h1>Overview</h1>
        <p className="view-sub">What your AI agents did on this machine.</p>
      </header>

      {props.liveSessions.length > 0 && (
        <section className="card live-card">
          <h3>Live now</h3>
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

      <div className="today-strip">
        <Stat label="sessions today" value={stats?.sessions_today} onOpen={() => props.onNavigate("timeline")} />
        <Stat label="commands today" value={stats?.commands_today} onOpen={() => props.onNavigate("timeline")} />
        <Stat label="installs today" value={stats?.packages_today} onOpen={() => props.onNavigate("packages")} />
        <Stat
          label="open flags"
          value={stats?.flagged_count}
          tone={stats && stats.flagged_count > 0 ? "bad" : undefined}
          onOpen={() => props.onNavigate("flagged")}
        />
      </div>

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
            <ul className="rows flush">
              {props.recentFlagged.slice(0, 5).map((e, i) => (
                <EventRow
                  key={e.id ?? i}
                  event={e}
                  showProject
                  onOpen={props.onOpenEvent}
                  action={
                    <button
                      className="row-action-btn ack"
                      title="Acknowledge"
                      aria-label="Acknowledge"
                      onClick={() => props.onAck(e)}
                    >
                      <CheckIcon size={14} />
                    </button>
                  }
                />
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
            <ul className="rows flush">
              {props.recentPackages.slice(0, 5).map((e, i) => (
                <EventRow key={e.id ?? i} event={e} showProject onOpen={props.onOpenEvent} />
              ))}
            </ul>
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
    <button className={`stat-pill${props.tone ? " tone-bad" : ""}`} onClick={props.onOpen}>
      <b>{props.value?.toLocaleString() ?? "-"}</b> {props.label}
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

function CaptureCard(props: { capture: CaptureStatus | null }) {
  const [openKey, setOpenKey] = useState<string | null>(null);
  const [copiedKey, setCopiedKey] = useState<string | null>(null);
  if (!props.capture) return null;
  const capture = props.capture;

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
        {CAPTURE_SOURCES.map((s) => {
          const count = sourceEventCount(capture, s);
          const live = count > 0;
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
                    ? `live · ${count} events`
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
