import { describe, expect, it } from "vitest";
import { familyOf, packageParts, type Family } from "./packages";
import { makeEvent } from "../test/fixtures/events";

function installEvent(summary: string, tool_name: string | null) {
  return makeEvent({ kind: "package_install", summary, tool_name });
}

describe("packageParts", () => {
  it("splits a pnpm add into manager and package names", () => {
    const parts = packageParts(installEvent("pnpm add p-retry zod", "pnpm"));
    expect(parts).toEqual({ manager: "pnpm", family: "js", names: ["p-retry", "zod"], appName: null });
  });

  it("keeps a pinned pip spec and drops flags", () => {
    const parts = packageParts(installEvent("pip install requests==2.32.0 -U", "pip"));
    expect(parts.family).toBe("py");
    expect(parts.names).toEqual(["requests==2.32.0"]);
  });

  it("skips the pip subcommand inside uv pip install", () => {
    const parts = packageParts(installEvent("uv pip install ruff", "uv"));
    expect(parts.manager).toBe("uv");
    expect(parts.family).toBe("py");
    expect(parts.names).toEqual(["ruff"]);
  });

  it("stops at && so the next command is not read as packages", () => {
    const parts = packageParts(installEvent("npm i -D vitest && pnpm build", "npm"));
    expect(parts.names).toEqual(["vitest"]);
  });

  it("stops at a stderr redirect", () => {
    const parts = packageParts(installEvent("brew install jq 2>/dev/null", "brew"));
    expect(parts.family).toBe("sys");
    expect(parts.names).toEqual(["jq"]);
  });

  it("stops at a stdout redirect and a pipe", () => {
    expect(packageParts(installEvent("brew install jq > log.txt", "brew")).names).toEqual(["jq"]);
    expect(packageParts(installEvent("brew install jq | tee log", "brew")).names).toEqual(["jq"]);
  });

  it("turns an application install summary into an app entry", () => {
    const parts = packageParts(installEvent("Application installed: Foo", null));
    expect(parts).toEqual({ manager: "app", family: "app", names: [], appName: "Foo" });
  });

  it("caps the chip list at six names", () => {
    const parts = packageParts(installEvent("pnpm add a b c d e f g h", "pnpm"));
    expect(parts.names).toEqual(["a", "b", "c", "d", "e", "f"]);
  });

  it("falls back to the first word when the event carries no tool name", () => {
    const parts = packageParts(installEvent("cargo install ripgrep", null));
    expect(parts.manager).toBe("cargo");
    expect(parts.family).toBe("rust");
    expect(parts.names).toEqual(["ripgrep"]);
  });
});

describe("familyOf", () => {
  const cases: [manager: string, family: Family][] = [
    ["npm", "js"],
    ["pnpm", "js"],
    ["yarn", "js"],
    ["bun", "js"],
    ["npx", "js"],
    ["pip", "py"],
    ["pip3", "py"],
    ["uv", "py"],
    ["poetry", "py"],
    ["pipx", "py"],
    ["cargo", "rust"],
    ["brew", "sys"],
    ["apt", "sys"],
    ["apt-get", "sys"],
    ["dnf", "sys"],
    ["yum", "sys"],
    ["snap", "sys"],
    ["winget", "sys"],
    ["choco", "sys"],
    ["mas", "sys"],
    ["app", "app"],
    ["gem", "other"],
    ["", "other"],
  ];

  it.each(cases)("%s is %s", (manager, family) => {
    expect(familyOf(manager)).toBe(family);
  });
});
