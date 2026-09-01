# Multi-Agent Integration Surfaces

Research date: 2026-08-29. Every major agent converged on the Claude Code hook shape (JSON config -> command gets JSON on stdin -> exit 0/2 + JSON stdout). Architecture: one normalized event schema, thin per-agent adapters.

## Normalized event model

```
{ agent, session_id, turn_id?, ts,
  event_kind: session_start|prompt|tool_call|tool_result|file_edit|shell_command|mcp_call|approval|session_end,
  payload, source: hook|log_tail|process|shim, capture_confidence }
```

Adapter = (a) installer that merges hooks non-destructively + verifies at startup (most agents read hook config only at launch), (b) log-tail watcher for reconciliation/backfill, (c) shims/process observation as last resort.

## Per-agent capture recipes

| Agent | Primary | Secondary | Notes |
|---|---|---|---|
| Claude Code | plugin + HTTP hooks | transcript JSONL + spool | see 04-claude-integration.md |
| Codex CLI | rollout tailing `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl` | hooks `~/.codex/hooks.json` | rollouts are complete replay logs incl. sandbox + approval policy per turn; hooks miss hosted tools so logs lead |
| Cursor | `~/.cursor/hooks.json` | `state.vscdb` SQLite (mind -wal) | Elastic-proven at 13M+ events / 1,100 machines |
| Gemini CLI | hooks in `~/.gemini/settings.json` | `~/.gemini/tmp/<hash>/{chats,logs}` + shadow-git checkpoints | could also BE its OTLP collector |
| Copilot CLI | `~/.copilot/hooks/*.json` | per-session `events.jsonl` | VS Code agent mode: chatSessions + Agent Debug Log parsing |
| Windsurf | `~/.codeium/windsurf/hooks.json` (12 Cascade events) | - | `post_cascade_response_with_transcript` emits JSONL transcript |
| OpenCode | TS plugin (`tool.execute.before/after` + event bus) | `~/.local/share/opencode/storage/` | plugin ships as npm pkg |
| Aider | tail `.aider.chat.history.md` + git-commit watcher (author =~ "aider") | `--llm-history-file` | no hooks; git IS the audit trail |
| Amp | toolbox/hooks + AMP_LOG_FILE | little local data | threads are server-side; weakest local story |

## Codex CLI detail (rollout order #2)

- Hooks (2026, near-clone of Claude Code): events SessionStart/End, UserPromptSubmit, Stop, Pre/PostCompact, Pre/PostToolUse, PermissionRequest, SubagentStart/Stop. Config: `hooks.json` or `[hooks]` in `config.toml`; repo -> `~/.codex` -> plugin -> MDM precedence. stdin: session_id, transcript_path, cwd, model, permission_mode, turn_id (+ tool fields). `async: true` background hooks. Limitation: PreToolUse only fires for Bash/apply_patch/MCP/local tools, not hosted tools.
- Rollouts: `~/.codex/sessions/.../rollout-*.jsonl`: session_meta, response_item (tool calls + outputs, call_id links), turn_context (records sandbox + approval policy per turn), event_msg. Format marked unstable: version-sniff via session_meta. Can reach 700MB-2GB. Respect `--ephemeral` / `[history] persistence="none"`.
- Also: `~/.codex/history.jsonl`, `~/.codex/log/codex-tui.log`, `codex exec --json` event stream, official opt-in OTel (`[otel]` block; emits tool approvals + invocation results).
- Docs: https://learn.chatgpt.com/docs/hooks · https://learn.chatgpt.com/docs/config-file/config-reference

## Always-on core (agent-agnostic)

Package-manager watcher regardless of adapter: lockfile diffing + optional PATH shims for npm/pnpm/pip/uv/cargo (Volta/asdf technique: log argv + parent PID, exec real binary). Installs are under-reported by every agent's tool events and packages are the #1 attack vector.

## Standardization landscape

- **ACP (Agent Client Protocol, Zed)**: JSON-RPC editor<->agent protocol, 50+ agents registered by Jun 2026; a client shim is a legit capture path but only for editor-mediated usage. https://zed.dev/acp
- OTel GenAI semantic conventions: still Development status, nothing stable. Export target, not foundation.
- No hook/log standard exists; de-facto convergence on the Claude shape (Cursor even exports CLAUDE_PROJECT_DIR).
- Parser prior art to mine: agentsview, agentgrep, coding_agent_session_search (26 agents -> normalized SQLite), simonw/claude-code-transcripts.

## Rollout order

1. Claude Code (v1) -> 2. Codex CLI -> 3. Cursor -> 4. Gemini CLI -> 5. Copilot CLI -> 6. OpenCode -> 7. Windsurf -> 8. Aider -> 9. Amp.
