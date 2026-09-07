import { useEffect, useMemo, useState } from "react";
import { api } from "../lib/api";
import type { AgentEvent } from "../lib/types";
// Rows open the app-wide slide-over (DetailPanel) instead of expanding
// inline; payload fetching lives there now.
import { agentCounts, agentLabel, groupByDay, projectName, severityOf, timeOf } from "../lib/format";
import { AgentChips } from "./AgentChips";
import { rowClass } from "./EventList";
import { CheckIcon, FlagIcon, TypeTile } from "./icons";
import { StatusBar } from "./StatusBar";

type Category =
  | "deletes"
  | "pipe"
  | "credentials"
  | "force-push"
  | "bypass"
  | "packages"
  | "other";

const CATEGORY_LABELS: { key: Category | "all"; label: string }[] = [
  { key: "all", label: "All" },
  { key: "deletes", label: "Deletes" },
  { key: "pipe", label: "Pipe to shell" },
  { key: "credentials", label: "Credentials" },
  { key: "force-push", label: "Force push" },
  { key: "bypass", label: "Permission bypass" },
  { key: "packages", label: "Packages" },
  { key: "other", label: "Other" },
];

export function FlaggedView(props: {
  flagged: AgentEvent[];
  ackedCount: number;
  selectedId?: number;
  onChanged: () => void;
  onOpenEvent: (event: AgentEvent, acked?: boolean) => void;
}) {
  const [bucket, setBucket] = useState<"open" | "acked">("open");
  const [ackedList, setAckedList] = useState<AgentEvent[]>([]);
  const [category, setCategory] = useState<Category | "all">("all");
  const [agent, setAgent] = useState("all");
  const [query, setQuery] = useState("");
  const [limit, setLimit] = useState(120);

  useEffect(() => {
    if (bucket !== "acked") return;
    api.flaggedEvents(true).then(setAckedList).catch(() => {});
  }, [bucket, props.ackedCount]);

  const source = bucket === "open" ? props.flagged : ackedList;
  const rows = useMemo(
    () => source.map((event) => ({ event, category: categoryOf(event.flag ?? "") })),
    [source],
  );

  const counts = useMemo(() => {
    const map = new Map<Category, number>();
    for (const row of rows) map.set(row.category, (map.get(row.category) ?? 0) + 1);
    return map;
  }, [rows]);

  const allFiltered = rows.filter((row) => {
    if (agent !== "all" && row.event.agent !== agent) return false;
    if (category !== "all" && row.category !== category) return false;
    if (!query) return true;
    const q = query.toLowerCase();
    return (
      (row.event.summary ?? "").toLowerCase().includes(q) ||
      (row.event.flag ?? "").toLowerCase().includes(q) ||
      projectName(row.event.cwd).toLowerCase().includes(q)
    );
  });
  // Rendering hundreds of rows at once is what makes the view feel heavy.
  const filtered = allFiltered.slice(0, limit);
  const hidden = allFiltered.length - filtered.length;

  const setAck = async (event: AgentEvent, acked: boolean) => {
    if (event.id === undefined) return;
    await api.ackEvent(event.id, acked).catch(() => {});
    if (bucket === "acked") {
      setAckedList((list) => list.filter((e) => e.id !== event.id));
    }
    props.onChanged();
  };

  return (
    <main className="view">
      <header className="view-head">
        <h1>Flagged</h1>
        <p className="view-sub">
          An inbox, not a graveyard: acknowledge what you've reviewed. Tracon
          flags; it never blocks.
        </p>
      </header>

      <div className="filterbar">
        <div className="seg">
          <button
            className={bucket === "open" ? "seg-item active" : "seg-item"}
            onClick={() => setBucket("open")}
          >
            Open {props.flagged.length}
          </button>
          <button
            className={bucket === "acked" ? "seg-item active" : "seg-item"}
            onClick={() => setBucket("acked")}
          >
            Acknowledged {props.ackedCount}
          </button>
        </div>
        <input
          className="search"
          type="search"
          placeholder="Search flagged commands, reasons, projects..."
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        <div className="filter-chips">
          {CATEGORY_LABELS.map((c) => {
            const count = c.key === "all" ? rows.length : (counts.get(c.key) ?? 0);
            if (c.key !== "all" && count === 0) return null;
            return (
              <button
                key={c.key}
                className={category === c.key ? "chip active chip-bad" : "chip"}
                onClick={() => setCategory(c.key)}
              >
                {c.label} <span className="chip-count">{count}</span>
              </button>
            );
          })}
        </div>
      </div>

      <AgentChips counts={agentCounts(source)} value={agent} onChange={setAgent} />

      {filtered.length === 0 ? (
        <div className="pkg-empty">
          {bucket === "open" && rows.length === 0 ? (
            <img className="mascot" src="/mascot/inbox-zero.png" alt="" />
          ) : (
            <FlagIcon size={34} />
          )}
          <p>
            {bucket === "acked"
              ? "Nothing acknowledged yet."
              : rows.length === 0
                ? "Inbox zero. Quiet skies."
                : "Nothing matches this filter."}
          </p>
        </div>
      ) : (
        groupByDay(filtered, (row) => row.event.ts).map((group) => (
          <section key={group.label} className="group">
            <h2 className="group-head">
              <span className="group-bar" />
              {group.label}
              <span className="group-count">{group.items.length}</span>
            </h2>
            <ul className="rows">
              {group.items.map((row, i) => {
                const sev = severityOf(row.event.flag ?? "", row.event.summary);
                const isOpen = bucket === "open";
                return (
                  <li key={row.event.id ?? i} className="row-line">
                    <button
                      className={rowClass(row.event, props.selectedId)}
                      onClick={() => props.onOpenEvent(row.event, !isOpen)}
                    >
                      <TypeTile kind={row.event.kind} toolName={row.event.tool_name} flagged />
                      <span className="row-main">
                        <span className="row-title">
                          {row.event.tool_name ?? row.event.kind}
                          <span className={`flag-chip sev-${sev}`}>{row.event.flag}</span>
                        </span>
                        <span className="row-sub">{row.event.summary}</span>
                      </span>
                      <span className="row-meta">
                        {projectName(row.event.cwd)} · {agentLabel(row.event.agent)} ·{" "}
                        {timeOf(row.event.ts)}
                      </span>
                    </button>
                    <button
                      className={isOpen ? "row-action-btn ack" : "row-action-btn"}
                      title={isOpen ? "Acknowledge" : "Reopen"}
                      aria-label={isOpen ? "Acknowledge" : "Reopen"}
                      onClick={() => setAck(row.event, isOpen)}
                    >
                      <CheckIcon size={14} />
                    </button>
                  </li>
                );
              })}
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
      <StatusBar left={`${filtered.length} of ${allFiltered.length} ${bucket === "open" ? "open" : "acknowledged"} flags shown`} />
    </main>
  );
}

function categoryOf(flag: string): Category {
  if (flag.includes("delete")) return "deletes";
  if (flag.includes("piped")) return "pipe";
  if (flag.includes("credential")) return "credentials";
  if (flag.includes("force push")) return "force-push";
  if (flag.includes("bypass")) return "bypass";
  if (flag.includes("vulnerabilit") || flag.includes("published")) return "packages";
  return "other";
}
