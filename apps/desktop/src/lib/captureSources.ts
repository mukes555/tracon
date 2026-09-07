import type { CaptureStatus } from "./types";

const CURSOR_HOOK_CMD =
  "sh -c 'cat | curl -s -m 5 -X POST -H \"content-type: application/json\" --data-binary @- http://localhost:48620/ingest -o /dev/null; exit 0'";

export type CaptureSource = {
  key: string;
  label: string;
  agent: string;
  source: string;
  /// Present when the source needs no setup; the text says why.
  auto?: string;
  how?: string;
  snippet?: string;
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
    snippet: `{ "version": 1, "hooks": { "beforeShellExecution": [{ "command": "${CURSOR_HOOK_CMD}" }] } }`,
  },
  {
    key: "gemini",
    label: "Gemini hooks",
    agent: "gemini",
    source: "hook",
    how: "Add BeforeTool/AfterTool command hooks in ~/.gemini/settings.json posting to the /ingest/gemini endpoint. Full snippet: integrations/gemini-hooks/README.md",
    snippet: "http://localhost:48620/ingest/gemini",
  },
];

export function sourceEventCount(capture: CaptureStatus | null, s: CaptureSource): number {
  const hit = capture?.counts.find((c) => c.agent === s.agent && c.source === s.source);
  return hit?.count ?? 0;
}
