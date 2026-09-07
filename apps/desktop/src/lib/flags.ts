/// One plain sentence per flag family: what the danger heuristic saw and
/// what to check. Keyed by substring because the Rust side sharpens the
/// wording (e.g. "known vulnerabilities in requests").
const WHY: [string, string][] = [
  ["credential", "Reads a file that usually holds secrets. Check whether the agent needed it and whether the contents went anywhere."],
  ["piped", "Downloads a script and runs it in one step, so nobody reviewed the code before it executed."],
  ["delete", "Removes files recursively. Confirm the path was the one you expected, not a parent of it."],
  ["force push", "Rewrites remote history. Commits other people pushed to that branch can be lost."],
  ["bypass", "Started an agent with permission prompts turned off, so nothing in that session asked you first."],
  ["vulnerabilit", "The installed version has a published security advisory. Upgrade or pin a fixed version."],
  ["published", "The package was published very recently, which is a common sign of typosquatting. Check the name carefully."],
  ["world-writable", "Made a file or directory writable by every user on the machine."],
  ["disk", "Touches a raw disk device. This can destroy a volume without a confirmation."],
];

export function flagWhy(flag: string): string {
  const hit = WHY.find(([needle]) => flag.includes(needle));
  return hit ? hit[1] : "Tracon's danger heuristics marked this action for a second look.";
}
