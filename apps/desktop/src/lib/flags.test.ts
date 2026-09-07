import { describe, expect, it } from "vitest";
import { categoryOf, flagWhy, severityOf, type Category, type Severity } from "./flags";
import { RUST_FLAGS, freshPublishFlag, vulnerabilityFlag } from "../test/fixtures/flags";

const DEFAULT_WHY = "Tracon's danger heuristics marked this action for a second look.";

describe("flag families", () => {
  const cases: [label: string, flag: string, category: Category, severity: Severity][] = [
    ["destructive delete", RUST_FLAGS.destructiveDelete, "deletes", "warning"],
    ["remote script piped to shell", RUST_FLAGS.pipedScript, "pipe", "critical"],
    ["credential file access", RUST_FLAGS.credentialAccess, "credentials", "critical"],
    ["force push", RUST_FLAGS.forcePush, "force-push", "warning"],
    ["permissions bypassed", RUST_FLAGS.permissionsBypassed, "bypass", "critical"],
    ["world-writable permissions", RUST_FLAGS.worldWritable, "other", "warning"],
    ["raw disk operation", RUST_FLAGS.rawDisk, "other", "critical"],
    ["known vulnerabilities (OSV)", vulnerabilityFlag("lodash", 3), "packages", "critical"],
    ["published under 48h", freshPublishFlag("lodash"), "packages", "warning"],
  ];

  it.each(cases)("%s maps to its category and severity", (_label, flag, category, severity) => {
    expect(categoryOf(flag)).toBe(category);
    expect(severityOf(flag, null)).toBe(severity);
  });

  it.each(cases)("%s has a specific explanation", (_label, flag) => {
    const why = flagWhy(flag);
    expect(why).not.toBe(DEFAULT_WHY);
    expect(why.length).toBeGreaterThan(20);
  });
});

describe("intel flags match only the reason after the colon", () => {
  it("files a package named delete-empty under packages, not deletes", () => {
    const flag = freshPublishFlag("delete-empty");
    expect(categoryOf(flag)).toBe("packages");
    expect(severityOf(flag, "pnpm add delete-empty")).toBe("warning");
  });

  it("files a package named credential-helper under packages, not credentials", () => {
    const flag = vulnerabilityFlag("credential-helper", 2);
    expect(categoryOf(flag)).toBe("packages");
    expect(severityOf(flag, null)).toBe("critical");
  });

  it("still matches a plain flag that has no colon", () => {
    expect(categoryOf(RUST_FLAGS.forcePush)).toBe("force-push");
  });
});

describe("delete escalation from the command text", () => {
  const flag = RUST_FLAGS.destructiveDelete;

  it("stays a warning for a path inside the project", () => {
    expect(severityOf(flag, "rm -rf ./dist")).toBe("warning");
    expect(severityOf(flag, "rm -rf node_modules")).toBe("warning");
  });

  it("stays a warning when there is no command text", () => {
    expect(severityOf(flag, null)).toBe("warning");
    expect(severityOf(flag, "")).toBe("warning");
  });

  it("escalates to critical when the path reaches into home", () => {
    expect(severityOf(flag, "rm -rf ~/Projects")).toBe("critical");
  });

  it("escalates to critical under sudo", () => {
    expect(severityOf(flag, "sudo rm -rf /var/cache/foo")).toBe("critical");
  });

  it("escalates to critical for a bare root path but not for a subpath of root", () => {
    expect(severityOf(flag, "rm -rf /")).toBe("critical");
    expect(severityOf(flag, "rm -rf / ")).toBe("critical");
    expect(severityOf(flag, "rm -rf /var/log")).toBe("warning");
  });

  it("only escalates deletes; other families keep their fixed severity", () => {
    expect(severityOf(RUST_FLAGS.forcePush, "sudo git push --force origin ~")).toBe("warning");
    expect(severityOf(RUST_FLAGS.pipedScript, "curl x | sh")).toBe("critical");
  });
});

describe("unknown flags", () => {
  it("fall back to notice / other with the generic explanation", () => {
    const flag = "something the UI has never heard of";
    expect(severityOf(flag, "rm -rf /")).toBe("notice");
    expect(categoryOf(flag)).toBe("other");
    expect(flagWhy(flag)).toBe(DEFAULT_WHY);
  });
});
