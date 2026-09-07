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

/** Only these agents keep a conversation transcript Tracon can read; the
    Conversation buttons hide for the rest instead of opening an empty view. */
export function hasTranscript(agent: string): boolean {
  return agent === "claude-code" || agent === "codex";
}

/** Per-agent counts for filter chips, largest first. */
export function agentCounts(items: { agent: string }[]): [string, number][] {
  const counts = new Map<string, number>();
  for (const item of items) counts.set(item.agent, (counts.get(item.agent) ?? 0) + 1);
  return [...counts.entries()].sort((a, b) => b[1] - a[1]);
}

export const FILE_TOOLS = new Set(["Edit", "Write", "MultiEdit", "NotebookEdit", "Read"]);

/** Plain words for what an event is, for row sublines. */
export function kindLabel(event: AgentEvent): string {
  if (event.kind === "prompt") return "Prompt";
  if (event.kind === "package_install") return `${event.tool_name ?? "package"} install`;
  if (event.kind === "tool_result") return "Result";
  if (event.kind === "session_start") return "Session started";
  if (event.kind === "session_end") return "Session ended";
  const tool = event.tool_name ?? "";
  if (tool === "Bash" || tool === "shell") return "Command";
  if (tool === "Read") return "File read";
  if (FILE_TOOLS.has(tool)) return "File edit";
  return tool || event.kind;
}

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

export function durationLabel(start: string, end: string): string {
  const ms = new Date(end).getTime() - new Date(start).getTime();
  if (!Number.isFinite(ms) || ms < 0) return "";
  const mins = Math.round(ms / 60000);
  if (mins < 1) return "under a minute";
  if (mins < 60) return `${mins} min`;
  return `${Math.floor(mins / 60)}h ${mins % 60}m`;
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
