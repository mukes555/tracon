import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  agentCounts,
  agentLabel,
  dayLabel,
  durationLabel,
  groupByDay,
  kindLabel,
  matchesKindFilter,
  matchesQuery,
  projectName,
  relTime,
} from "./format";
import type { KindFilter } from "./types";
import { makeEvent } from "../test/fixtures/events";

// Every date-relative helper compares against "now", so the clock is frozen
// at local noon and timestamps are built relative to it.
const NOW = new Date(2026, 8, 8, 12, 0, 0);

function localDate(daysAgo: number): string {
  return new Date(NOW.getFullYear(), NOW.getMonth(), NOW.getDate() - daysAgo, 9, 30).toISOString();
}

function secondsAgo(seconds: number): string {
  return new Date(NOW.getTime() - seconds * 1000).toISOString();
}

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(NOW);
});

afterEach(() => {
  vi.useRealTimers();
});

describe("kindLabel", () => {
  it("names prompts, results and session markers", () => {
    expect(kindLabel(makeEvent({ kind: "prompt" }))).toBe("Prompt");
    expect(kindLabel(makeEvent({ kind: "tool_result" }))).toBe("Result");
    expect(kindLabel(makeEvent({ kind: "session_start" }))).toBe("Session started");
    expect(kindLabel(makeEvent({ kind: "session_end" }))).toBe("Session ended");
  });

  it("calls Bash and shell tool calls commands", () => {
    expect(kindLabel(makeEvent({ tool_name: "Bash" }))).toBe("Command");
    expect(kindLabel(makeEvent({ tool_name: "shell" }))).toBe("Command");
  });

  it("separates reads from edits among file tools", () => {
    expect(kindLabel(makeEvent({ tool_name: "Edit" }))).toBe("File edit");
    expect(kindLabel(makeEvent({ tool_name: "Write" }))).toBe("File edit");
    expect(kindLabel(makeEvent({ tool_name: "Read" }))).toBe("File read");
  });

  it("labels package installs by manager, with a fallback when unknown", () => {
    expect(kindLabel(makeEvent({ kind: "package_install", tool_name: "pnpm" }))).toBe("pnpm install");
    expect(kindLabel(makeEvent({ kind: "package_install", tool_name: null }))).toBe("package install");
  });

  it("falls back to the tool name, then the kind", () => {
    expect(kindLabel(makeEvent({ tool_name: "WebFetch" }))).toBe("WebFetch");
    expect(kindLabel(makeEvent({ kind: "mystery", tool_name: null }))).toBe("mystery");
  });
});

describe("dayLabel", () => {
  it("says Today and Yesterday for the two most recent days", () => {
    expect(dayLabel(localDate(0))).toBe("Today");
    expect(dayLabel(localDate(1))).toBe("Yesterday");
  });

  it("shows a short date for anything older", () => {
    const label = dayLabel(localDate(8));
    expect(label).not.toBe("Today");
    expect(label).not.toBe("Yesterday");
    expect(label).toContain("31");
  });

  it("treats a future timestamp as Today", () => {
    expect(dayLabel(localDate(-1))).toBe("Today");
  });

  it("buckets an unparseable timestamp as Earlier", () => {
    expect(dayLabel("not a date")).toBe("Earlier");
  });
});

describe("groupByDay", () => {
  it("groups consecutive items that share a day", () => {
    const items = [localDate(0), localDate(0), localDate(1), localDate(8), localDate(8)];
    const groups = groupByDay(items, (ts) => ts);
    expect(groups.map((g) => g.label)).toEqual(["Today", "Yesterday", dayLabel(localDate(8))]);
    expect(groups.map((g) => g.items.length)).toEqual([2, 1, 2]);
  });

  it("returns no groups for no items", () => {
    expect(groupByDay([], (ts: string) => ts)).toEqual([]);
  });

  it("only merges neighbours, so unsorted input yields split groups", () => {
    const items = [localDate(0), localDate(1), localDate(0)];
    const labels = groupByDay(items, (ts) => ts).map((g) => g.label);
    expect(labels).toEqual(["Today", "Yesterday", "Today"]);
  });
});

describe("relTime", () => {
  it("rounds anything under 45 seconds to just now", () => {
    expect(relTime(secondsAgo(0))).toBe("just now");
    expect(relTime(secondsAgo(44))).toBe("just now");
  });

  it("calls 45 to 89 seconds one minute", () => {
    expect(relTime(secondsAgo(46))).toBe("1 min ago");
    expect(relTime(secondsAgo(89))).toBe("1 min ago");
  });

  it("switches to whole minutes from 90 seconds", () => {
    expect(relTime(secondsAgo(91))).toBe("1 min ago");
    expect(relTime(secondsAgo(120))).toBe("2 min ago");
    expect(relTime(secondsAgo(59 * 60))).toBe("59 min ago");
  });

  it("switches to hours from 60 minutes", () => {
    expect(relTime(secondsAgo(61 * 60))).toBe("1h ago");
    expect(relTime(secondsAgo(3 * 3600 + 59 * 60))).toBe("3h ago");
  });

  it("treats future and unparseable timestamps as just now", () => {
    expect(relTime(secondsAgo(-30))).toBe("just now");
    expect(relTime("garbage")).toBe("just now");
  });
});

describe("durationLabel", () => {
  const start = secondsAgo(0);
  const after = (seconds: number) => new Date(NOW.getTime() + seconds * 1000).toISOString();

  it("rounds short sessions to under a minute", () => {
    expect(durationLabel(start, after(0))).toBe("under a minute");
    expect(durationLabel(start, after(29))).toBe("under a minute");
  });

  it("shows minutes under an hour", () => {
    expect(durationLabel(start, after(31))).toBe("1 min");
    expect(durationLabel(start, after(5 * 60))).toBe("5 min");
    expect(durationLabel(start, after(59 * 60))).toBe("59 min");
  });

  it("shows hours and remaining minutes", () => {
    expect(durationLabel(start, after(60 * 60))).toBe("1h 0m");
    expect(durationLabel(start, after(90 * 60))).toBe("1h 30m");
    expect(durationLabel(start, after(25 * 3600 + 5 * 60))).toBe("25h 5m");
  });

  it("is empty when the end precedes the start or a side is unparseable", () => {
    expect(durationLabel(after(60), start)).toBe("");
    expect(durationLabel("nope", after(60))).toBe("");
    expect(durationLabel(start, "nope")).toBe("");
  });
});

describe("matchesKindFilter", () => {
  const command = makeEvent({ tool_name: "Bash" });
  const shell = makeEvent({ tool_name: "shell" });
  const edit = makeEvent({ tool_name: "Edit" });
  const read = makeEvent({ tool_name: "Read" });
  const install = makeEvent({ kind: "package_install", tool_name: "pnpm" });
  const prompt = makeEvent({ kind: "prompt" });
  const flagged = makeEvent({ tool_name: "Bash", flag: "force push" });
  const bashResult = makeEvent({ kind: "tool_result", tool_name: "Bash" });

  it("all accepts everything", () => {
    for (const event of [command, edit, install, prompt, flagged, bashResult]) {
      expect(matchesKindFilter(event, "all")).toBe(true);
    }
  });

  it("commands accepts Bash and shell tool calls only", () => {
    expect(matchesKindFilter(command, "commands")).toBe(true);
    expect(matchesKindFilter(shell, "commands")).toBe(true);
    expect(matchesKindFilter(edit, "commands")).toBe(false);
    expect(matchesKindFilter(bashResult, "commands")).toBe(false);
  });

  it("files accepts file tool calls including reads", () => {
    expect(matchesKindFilter(edit, "files")).toBe(true);
    expect(matchesKindFilter(read, "files")).toBe(true);
    expect(matchesKindFilter(command, "files")).toBe(false);
  });

  it("packages and prompts match on kind", () => {
    expect(matchesKindFilter(install, "packages")).toBe(true);
    expect(matchesKindFilter(command, "packages")).toBe(false);
    expect(matchesKindFilter(prompt, "prompts")).toBe(true);
    expect(matchesKindFilter(command, "prompts")).toBe(false);
  });

  it("flagged accepts any event carrying a flag", () => {
    expect(matchesKindFilter(flagged, "flagged")).toBe(true);
    expect(matchesKindFilter(command, "flagged")).toBe(false);
  });

  it("covers every filter value", () => {
    const filters: KindFilter[] = ["all", "commands", "files", "packages", "prompts", "flagged"];
    for (const filter of filters) {
      expect(typeof matchesKindFilter(command, filter)).toBe("boolean");
    }
  });
});

describe("matchesQuery", () => {
  const event = makeEvent({ summary: "git push --force", tool_name: "Bash", flag: "force push" });

  it("accepts everything for an empty query", () => {
    expect(matchesQuery(event, "")).toBe(true);
  });

  it("matches case-insensitively across summary, tool name and flag", () => {
    expect(matchesQuery(event, "PUSH")).toBe(true);
    expect(matchesQuery(event, "bash")).toBe(true);
    expect(matchesQuery(event, "force push")).toBe(true);
    expect(matchesQuery(event, "cargo")).toBe(false);
  });
});

describe("agentLabel", () => {
  it("maps known agent ids to display names", () => {
    expect(agentLabel("claude-code")).toBe("Claude");
    expect(agentLabel("codex")).toBe("Codex");
    expect(agentLabel("cursor")).toBe("Cursor");
    expect(agentLabel("gemini")).toBe("Gemini");
    expect(agentLabel("system")).toBe("System");
  });

  it("passes unknown agent ids through untouched", () => {
    expect(agentLabel("aider")).toBe("aider");
  });
});

describe("agentCounts", () => {
  it("counts per agent, largest first", () => {
    const items = [{ agent: "codex" }, { agent: "claude-code" }, { agent: "claude-code" }];
    expect(agentCounts(items)).toEqual([
      ["claude-code", 2],
      ["codex", 1],
    ]);
  });
});

describe("projectName", () => {
  it("uses the last path segment", () => {
    expect(projectName("/Users/me/Projects/tracon")).toBe("tracon");
    expect(projectName("/Users/me/Projects/tracon/")).toBe("tracon");
  });

  it("understands Windows backslash paths", () => {
    expect(projectName("C:\\Users\\me\\Projects\\tracon")).toBe("tracon");
    expect(projectName("C:\\Users\\me\\mixed/tracon")).toBe("tracon");
  });

  it("falls back for missing or root paths", () => {
    expect(projectName(null)).toBe("unknown project");
    expect(projectName("/")).toBe("/");
  });
});
