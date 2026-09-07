import type { AgentEvent } from "./types";

export type Family = "js" | "py" | "rust" | "sys" | "app" | "other";

export type PackageParts = {
  manager: string;
  family: Family;
  names: string[];
  appName: string | null;
};

const SKIP_WORDS = new Set(["install", "add", "i", "pip"]);
const STOP_WORDS = new Set(["&&", "||", "|", ";", ">", ">>", "2>&1"]);

/** Splits an install summary into the manager and the package names, so a
    row can show chips instead of repeating the raw command. */
export function packageParts(event: AgentEvent): PackageParts {
  const summary = event.summary ?? "";
  if (summary.startsWith("Application installed: ")) {
    return {
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
  return { manager, family: familyOf(manager), names, appName: null };
}

export function familyOf(manager: string): Family {
  if (["npm", "pnpm", "yarn", "bun", "npx"].includes(manager)) return "js";
  if (["pip", "pip3", "uv", "poetry", "pipx"].includes(manager)) return "py";
  if (manager === "cargo") return "rust";
  if (["brew", "apt", "apt-get", "dnf", "yum", "snap", "winget", "choco", "mas"].includes(manager)) {
    return "sys";
  }
  if (manager === "app") return "app";
  return "other";
}
