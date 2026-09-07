/// Everything the UI knows about a flag family lives in this one table:
/// how to recognise the Rust side's flag string, how to rank it, which
/// filter bucket it belongs to, and one sentence on why it matters.
export type Severity = "critical" | "warning" | "notice";

export type Category =
  | "deletes"
  | "pipe"
  | "credentials"
  | "force-push"
  | "bypass"
  | "packages"
  | "other";

type Family = { needle: string; severity: Severity; category: Category; why: string };

const FAMILIES: Family[] = [
  {
    needle: "credential",
    severity: "critical",
    category: "credentials",
    why: "Reads a file that usually holds secrets. Check whether the agent needed it and whether the contents went anywhere.",
  },
  {
    needle: "piped",
    severity: "critical",
    category: "pipe",
    why: "Downloads a script and runs it in one step, so nobody reviewed the code before it executed.",
  },
  {
    needle: "delete",
    severity: "warning",
    category: "deletes",
    why: "Removes files recursively. Confirm the path was the one you expected, not a parent of it.",
  },
  {
    needle: "force push",
    severity: "warning",
    category: "force-push",
    why: "Rewrites remote history. Commits other people pushed to that branch can be lost.",
  },
  {
    needle: "bypass",
    severity: "critical",
    category: "bypass",
    why: "Started an agent with permission prompts turned off, so nothing in that session asked you first.",
  },
  {
    needle: "vulnerabilit",
    severity: "critical",
    category: "packages",
    why: "The installed version has a published security advisory. Upgrade or pin a fixed version.",
  },
  {
    needle: "published",
    severity: "warning",
    category: "packages",
    why: "The package was published very recently, which is a common sign of typosquatting. Check the name carefully.",
  },
  {
    needle: "world-writable",
    severity: "warning",
    category: "other",
    why: "Made a file or directory writable by every user on the machine.",
  },
  {
    needle: "disk",
    severity: "critical",
    category: "other",
    why: "Touches a raw disk device. This can destroy a volume without a confirmation.",
  },
];

/// Intel flags read "{package}: {reason}"; only the reason is matched, so a
/// package called delete-empty is not filed under deletes.
function familyOf(flag: string): Family | undefined {
  const colon = flag.indexOf(": ");
  const reason = colon >= 0 ? flag.slice(colon + 2) : flag;
  return FAMILIES.find((f) => reason.includes(f.needle));
}

/** Tiers a flag; the command text sharpens deletes that reach outside the
    project (home, sudo, or a bare root path). */
export function severityOf(flag: string, summary: string | null): Severity {
  const family = familyOf(flag);
  if (!family) return "notice";
  if (family.category !== "deletes") return family.severity;
  const s = summary ?? "";
  const reachesFar = s.includes("~") || s.includes("sudo") || / \/(\s|$)/.test(s);
  return reachesFar ? "critical" : "warning";
}

export function categoryOf(flag: string): Category {
  return familyOf(flag)?.category ?? "other";
}

export function flagWhy(flag: string): string {
  return familyOf(flag)?.why ?? "Tracon's danger heuristics marked this action for a second look.";
}
