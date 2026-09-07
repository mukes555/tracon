import type { CaptureStatus } from "./types";

const INGEST_URL = "http://127.0.0.1:48620/ingest";
const GEMINI_URL = "http://127.0.0.1:48620/ingest/gemini";

// The hook snippets must match the shell the user's agent runs them in;
// Windows agents run their hooks through PowerShell, everything else sh.
const ON_WINDOWS = typeof navigator !== "undefined" && /Windows/i.test(navigator.userAgent);

function shHook(url: string): string {
  return (
    "sh -c 'cat | curl -s -m 5 -X POST -H \"content-type: application/json\" " +
    '-H "X-Tracon-Token: $(cat ~/.tracon/token 2>/dev/null)" ' +
    `--data-binary @- ${url} -o /dev/null; exit 0'`
  );
}

function powershellHook(url: string): string {
  return (
    'powershell -NoProfile -Command "$b=[Console]::In.ReadToEnd(); ' +
    `Invoke-RestMethod -Method Post -Uri ${url} -ContentType application/json ` +
    "-Headers @{'X-Tracon-Token'=(Get-Content $env:USERPROFILE\\.tracon\\token)} -Body $b | Out-Null\""
  );
}

function hookCommand(url: string): string {
  return ON_WINDOWS ? powershellHook(url) : shHook(url);
}

export type CaptureSource = {
  key: string;
  label: string;
  agent: string;
  source: string;
  /// Present when the source needs no setup; the text says why.
  auto?: string;
  how?: string;
  snippet?: string;
  /// When the probe says the agent is not installed, this text replaces
  /// the "not connected" line so the user does not chase setup for nothing.
  installed?: (capture: CaptureStatus) => boolean;
  missing?: string;
};

/// The one registry of where events can come from. Overview renders the
/// setup instructions from it; the inspector renders liveness from it.
export const CAPTURE_SOURCES: CaptureSource[] = [
  {
    key: "claude-hooks",
    label: "Claude Code hooks",
    agent: "claude-code",
    source: "hook",
    how: "Real-time capture. Run Claude Code with the Tracon plugin:",
    snippet: "claude --plugin-dir <tracon-repo>/integrations/claude-plugin",
  },
  {
    key: "claude-tail",
    label: "Claude Code transcripts",
    agent: "claude-code",
    source: "log_tail",
    auto: "automatic: read-only tailing of ~/.claude/projects",
  },
  {
    key: "codex",
    label: "Codex rollouts",
    agent: "codex",
    source: "log_tail",
    auto: "automatic when Codex CLI is installed (~/.codex/sessions)",
  },
  {
    key: "cursor",
    label: "Cursor hooks",
    agent: "cursor",
    source: "hook",
    how: "Merge this hook into ~/.cursor/hooks.json (events: beforeShellExecution, afterFileEdit, beforeSubmitPrompt...), then restart Cursor:",
    // Built from an object so the quotes inside the command are escaped
    // exactly as the JSON file needs them.
    snippet: JSON.stringify({
      version: 1,
      hooks: { beforeShellExecution: [{ command: hookCommand(INGEST_URL) }] },
    }),
    installed: (c) => c.cursor_found,
    missing: "Cursor not found",
  },
  {
    key: "gemini",
    label: "Gemini hooks",
    agent: "gemini",
    source: "hook",
    how: "Add BeforeTool/AfterTool command hooks in ~/.gemini/settings.json running this command. Full snippet: integrations/gemini-hooks/README.md",
    snippet: hookCommand(GEMINI_URL),
    installed: (c) => c.gemini_found,
    missing: "Gemini CLI not found",
  },
];

export function sourceEventCount(capture: CaptureStatus | null, s: CaptureSource): number {
  const hit = capture?.counts.find((c) => c.agent === s.agent && c.source === s.source);
  return hit?.count ?? 0;
}

/** The red line for a recorder that is receiving events but failing to
    store them; null while everything is being saved. */
export function ingestFailureText(capture: CaptureStatus | null): string | null {
  if (!capture || capture.insert_failures === 0) return null;
  const n = capture.insert_failures;
  const head = `${n} ${n === 1 ? "event" : "events"} could not be saved.`;
  return capture.last_error ? `${head} Last error: ${capture.last_error}` : head;
}
