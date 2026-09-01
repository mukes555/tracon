import { useMemo, useState } from "react";
import type { AgentEvent } from "../lib/types";
// Rows open the app-wide slide-over (DetailPanel); payload fetching lives there.
import { agentCounts, agentLabel, groupByDay, projectName, timeOf } from "../lib/format";
import { AgentChips } from "./AgentChips";
import { BoxIcon } from "./icons";

type Family = "js" | "py" | "rust" | "sys" | "app" | "other";

type PackageRow = {
  event: AgentEvent;
  manager: string;
  family: Family;
  names: string[];
  appName: string | null;
};

const FAMILY_LABELS: { key: Family | "all"; label: string }[] = [
  { key: "all", label: "All" },
  { key: "js", label: "JavaScript" },
  { key: "py", label: "Python" },
  { key: "rust", label: "Rust" },
  { key: "sys", label: "System" },
  { key: "app", label: "Apps" },
  { key: "other", label: "Other" },
];

export function PackagesView(props: {
  packages: AgentEvent[];
  intelEnabled: boolean | null;
  onGoSettings: () => void;
  onOpenEvent: (event: AgentEvent) => void;
}) {
  const [family, setFamily] = useState<Family | "all">("all");
  const [agent, setAgent] = useState("all");
  const [query, setQuery] = useState("");
  const [limit, setLimit] = useState(120);

  const rows = useMemo(() => props.packages.map(toRow), [props.packages]);

  const familyCounts = useMemo(() => {
    const counts = new Map<Family, number>();
    for (const row of rows) counts.set(row.family, (counts.get(row.family) ?? 0) + 1);
    return counts;
  }, [rows]);

  const allFiltered = rows.filter((row) => {
    if (agent !== "all" && row.event.agent !== agent) return false;
    if (family !== "all" && row.family !== family) return false;
    if (!query) return true;
    const q = query.toLowerCase();
    return (
      (row.event.summary ?? "").toLowerCase().includes(q) ||
      row.names.some((n) => n.toLowerCase().includes(q)) ||
      projectName(row.event.cwd).toLowerCase().includes(q)
    );
  });
  // Rendering hundreds of rows at once is what makes the view feel heavy.
  const filtered = allFiltered.slice(0, limit);
  const hidden = allFiltered.length - filtered.length;

  return (
    <main className="view">
      <header className="view-head">
        <h1>Packages</h1>
        <p className="view-sub">
          Everything your agents installed, plus apps that appeared on this machine.
        </p>
        <IntelStatus enabled={props.intelEnabled} onGoSettings={props.onGoSettings} />
      </header>

      <div className="filterbar">
        <input
          className="search"
          type="search"
          placeholder="Search packages, commands, projects..."
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        <div className="filter-chips">
          {FAMILY_LABELS.map((f) => {
            const count = f.key === "all" ? rows.length : (familyCounts.get(f.key) ?? 0);
            if (f.key !== "all" && count === 0) return null;
            return (
              <button
                key={f.key}
                className={family === f.key ? "chip active" : "chip"}
                onClick={() => setFamily(f.key)}
              >
                {f.label} <span className="chip-count">{count}</span>
              </button>
            );
          })}
        </div>
      </div>

      <AgentChips
        counts={agentCounts(props.packages)}
        value={agent}
        onChange={setAgent}
      />

      {filtered.length === 0 ? (
        <div className="pkg-empty">
          <BoxIcon size={34} />
          <p>{rows.length === 0 ? "No package installs recorded yet." : "Nothing matches this filter."}</p>
          {rows.length === 0 && (
            <p className="muted">
              The next time an agent runs npm, pip, cargo, brew, or friends, or an
              app lands in your Applications folder, it shows up here.
            </p>
          )}
        </div>
      ) : (
        groupByDay(filtered, (row) => row.event.ts).map((group) => (
          <section key={group.label} className="pkg-group">
            <h2>{group.label}</h2>
            <ul className="pkg-list">
              {group.items.map((row, i) => (
                <li key={row.event.id ?? i}>
                  <button
                    className={`pkg-row${row.event.flag ? " flagged" : ""}`}
                    onClick={() => props.onOpenEvent(row.event)}
                  >
                    <span className={`pkg-badge fam-${row.family}`}>{row.manager}</span>
                    <span className="pkg-main">
                      <span className="pkg-names">
                        {row.appName ? (
                          <span className="pkg-chip app-chip">{row.appName}</span>
                        ) : row.names.length > 0 ? (
                          row.names.map((n) => (
                            <span key={n} className="pkg-chip">
                              {n}
                            </span>
                          ))
                        ) : (
                          <span className="pkg-lockfile">install from lockfile</span>
                        )}
                        {row.event.flag && <span className="flag-chip">{row.event.flag}</span>}
                      </span>
                      <span className="pkg-meta">
                        {projectName(row.event.cwd)} · {agentLabel(row.event.agent)} ·{" "}
                        {timeOf(row.event.ts)}
                      </span>
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          </section>
        ))
      )}
      {hidden > 0 && (
        <div className="list-more">
          <button className="thread-btn" onClick={() => setLimit(Number.POSITIVE_INFINITY)}>
            Show {hidden} more
          </button>
        </div>
      )}
    </main>
  );
}

function IntelStatus(props: { enabled: boolean | null; onGoSettings: () => void }) {
  if (props.enabled === null) return null;
  return (
    <span className="intel-status">
      <span className={props.enabled ? "dot ok" : "dot"} />
      {props.enabled ? (
        "threat intel on"
      ) : (
        <>
          threat intel off ·{" "}
          <button className="linkish" onClick={props.onGoSettings}>
            enable
          </button>
        </>
      )}
    </span>
  );
}

const SKIP_WORDS = new Set(["install", "add", "i", "pip"]);
const STOP_WORDS = new Set(["&&", "||", "|", ";", ">", ">>", "2>&1"]);

function toRow(event: AgentEvent): PackageRow {
  const summary = event.summary ?? "";
  if (summary.startsWith("Application installed: ")) {
    return {
      event,
      manager: "app",
      family: "app",
      names: [],
      appName: summary.slice("Application installed: ".length),
    };
  }

  const words = summary.split(/\s+/);
  const manager = event.tool_name ?? words[0] ?? "pkg";
  const names: string[] = [];
  for (const word of words.slice(1)) {
    if (STOP_WORDS.has(word) || word.startsWith("2>") || word.startsWith(">")) break;
    if (word.startsWith("-") || SKIP_WORDS.has(word)) continue;
    names.push(word);
    if (names.length >= 6) break;
  }
  return { event, manager, family: familyOf(manager), names, appName: null };
}

function familyOf(manager: string): Family {
  if (["npm", "pnpm", "yarn", "bun", "npx"].includes(manager)) return "js";
  if (["pip", "pip3", "uv", "poetry", "pipx"].includes(manager)) return "py";
  if (manager === "cargo") return "rust";
  if (["brew", "apt", "apt-get", "dnf", "yum", "snap", "winget", "choco", "mas"].includes(manager)) {
    return "sys";
  }
  if (manager === "app") return "app";
  return "other";
}
