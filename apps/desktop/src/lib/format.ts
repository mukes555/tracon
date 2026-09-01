import type { AgentEvent, KindFilter } from "./types";

export function projectName(cwd: string | null): string {
  if (!cwd) return "unknown project";
  const parts = cwd.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? cwd;
}

/** Today / Yesterday / "Aug 31" for list group headers. */
export function dayLabel(ts: string): string {
  const d = new Date(ts);
  if (isNaN(d.getTime())) return "Earlier";
  const today = new Date();
  const startOf = (x: Date) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
  const diffDays = Math.round((startOf(today) - startOf(d)) / 86400000);
  if (diffDays <= 0) return "Today";
  if (diffDays === 1) return "Yesterday";
  return d.toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

/** Group consecutive items sharing a calendar day, labeled via dayLabel.
    Items must already be sorted by time. */
export function groupByDay<T>(
  items: T[],
  tsOf: (item: T) => string,
): { label: string; items: T[] }[] {
  const groups: { label: string; items: T[] }[] = [];
  for (const item of items) {
    const label = dayLabel(tsOf(item));
    const last = groups[groups.length - 1];
    if (last && last.label === label) {
      last.items.push(item);
    } else {
      groups.push({ label, items: [item] });
    }
  }
  return groups;
}

/** Time for today's events, date + time for older ones. */
export function timeOf(ts: string): string {
  const d = new Date(ts);
  if (isNaN(d.getTime())) return ts;
  const today = new Date();
  const sameDay =
    d.getFullYear() === today.getFullYear() &&
    d.getMonth() === today.getMonth() &&
    d.getDate() === today.getDate();
  if (sameDay) return d.toLocaleTimeString();
  return d.toLocaleDateString(undefined, { month: "short", day: "numeric" }) +
    " " +
    d.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
}

export function agentLabel(agent: string): string {
  if (agent === "claude-code") return "Claude";
  if (agent === "codex") return "Codex";
  if (agent === "cursor") return "Cursor";
  if (agent === "gemini") return "Gemini";
  if (agent === "system") return "System";
  return agent;
}

/** Per-agent counts for filter chips, largest first. */
export function agentCounts(items: { agent: string }[]): [string, number][] {
  const counts = new Map<string, number>();
  for (const item of items) counts.set(item.agent, (counts.get(item.agent) ?? 0) + 1);
  return [...counts.entries()].sort((a, b) => b[1] - a[1]);
}

const FILE_TOOLS = new Set(["Edit", "Write", "MultiEdit", "NotebookEdit", "Read"]);

export function matchesKindFilter(event: AgentEvent, filter: KindFilter): boolean {
  switch (filter) {
    case "all":
      return true;
    case "commands":
      return event.kind === "tool_call" && (event.tool_name === "Bash" || event.tool_name === "shell");
    case "files":
      return event.kind === "tool_call" && FILE_TOOLS.has(event.tool_name ?? "");
    case "packages":
      return event.kind === "package_install";
    case "prompts":
      return event.kind === "prompt";
    case "flagged":
      return event.flag !== null;
  }
}

export function matchesQuery(event: AgentEvent, query: string): boolean {
  if (!query) return true;
  const q = query.toLowerCase();
  return (
    (event.summary ?? "").toLowerCase().includes(q) ||
    (event.tool_name ?? "").toLowerCase().includes(q) ||
    (event.flag ?? "").toLowerCase().includes(q)
  );
}

export type Severity = "critical" | "warning" | "notice";

/** Tiers our own flag strings; summaries sharpen destructive deletes. */
export function severityOf(flag: string, summary: string | null): Severity {
  if (
    flag.includes("credential") ||
    flag.includes("piped") ||
    flag.includes("bypass") ||
    flag.includes("disk") ||
    flag.includes("vulnerabilit")
  ) {
    return "critical";
  }
  if (flag.includes("delete")) {
    const s = summary ?? "";
    if (s.includes("~") || s.includes("sudo") || / \/(\s|$)/.test(s)) return "critical";
    return "warning";
  }
  if (flag.includes("force push") || flag.includes("world-writable") || flag.includes("published")) {
    return "warning";
  }
  return "notice";
}

export function relTime(ts: string): string {
  const ms = Date.now() - new Date(ts).getTime();
  if (!Number.isFinite(ms) || ms < 0) return "just now";
  const s = Math.floor(ms / 1000);
  if (s < 45) return "just now";
  if (s < 90) return "1 min ago";
  const m = Math.floor(s / 60);
  if (m < 60) return `${m} min ago`;
  return `${Math.floor(m / 60)}h ago`;
}
