# Claude Code + Claude Desktop Integration (Priority 1)

Research date: 2026-08-29, incl. live inspection of a real ~/.claude install (Claude Code v2.1.226).
Decision: ship a Claude Code PLUGIN carrying HTTP hooks to Tracon's localhost server; transcripts as secondary; OTel as fallback; MCP for queries only.

## Hooks (docs: https://code.claude.com/docs/en/hooks)

~31 events as of 2026. Audit-relevant set:
`PreToolUse, PostToolUse, PostToolUseFailure, PermissionRequest, PermissionDenied, UserPromptSubmit, SessionStart, SessionEnd, SubagentStart, SubagentStop, Stop, PreCompact, ConfigChange` (ConfigChange fires on settings edits = tamper signal).

Common stdin/POST payload: `session_id`, `prompt_id` (v2.1.196+, correlates with OTel), `transcript_path`, `cwd`, `permission_mode` (incl. bypassPermissions!), `hook_event_name`, `agent_id`/`agent_type` in subagents. Tool events add `tool_name` (MCP tools as `mcp__server__tool`), `tool_input` (full Bash command string, file paths), `tool_use_id`, and `tool_response` on Post* events.

Hook types: `command`, `http`, `mcp_tool`, `prompt`, `agent`.

**HTTP hooks are ideal**: `{"type":"http","url":"http://localhost:PORT/ingest","timeout":5}`.
Failure semantics (critical): non-2xx = non-blocking, connection refused = non-blocking, timeout = canceled + action proceeds. **A dead endpoint never blocks the agent.** No retry: fire-once (hence the spool-file backup).

Command hooks support `async: true` (background, non-blocking): use for the NDJSON spool writer.

Blocking: exit 2 blocks (PreToolUse etc.); we never use it in v1 (pure observer). Always set small `timeout` (2-5s); default is 600s. SessionEnd hooks share a 1.5s budget: no heavy work there.

Matchers: `"*"` all; names/pipes/regex; `if` field takes permission-rule syntax (`"Bash(rm *)"`).

## Config locations and merge

- `~/.claude/settings.json` (user), `.claude/settings.json` (project), `.claude/settings.local.json`, managed policy (`/Library/Application Support/ClaudeCode/managed-settings.json` on macOS, `C:\Program Files\ClaudeCode\` on Windows), plugin `hooks/hooks.json`, skill/agent frontmatter.
- **Hook arrays from all sources MERGE (accumulate), never replace.** Installing ours can't clobber user hooks. Settings are hot-watched (no restart needed).
- Blockers: `disableAllHooks`, `allowManagedHooksOnly`, `allowedHttpHookUrls` allowlist, `strictKnownMarketplaces`. Mitigation: detect + degrade to transcript tailing; document a managed-settings recipe for enterprises (also makes Tracon user-tamper-proof = team-tier feature).

## Plugin system (distribution vehicle)

```
tracon-plugin/
  .claude-plugin/plugin.json      # name, version, author
  hooks/hooks.json                # same schema as settings "hooks" block
  .mcp.json                       # bundled MCP query server
  bin/                            # executables on PATH while enabled
```
- `${CLAUDE_PLUGIN_ROOT}` = install dir; `${CLAUDE_PLUGIN_DATA}` = persistent data dir.
- Marketplace: repo with `.claude-plugin/marketplace.json`; users: `/plugin marketplace add ourorg/tracon` then `/plugin install`. Background auto-update on version bump. Also submit to anthropics/claude-plugins-community.
- Dev loop: `claude --plugin-dir ./tracon-plugin`, `/reload-plugins`.

## Transcripts (secondary source)

`~/.claude/projects/<cwd-encoded>/<session-uuid>.jsonl`: record types `assistant/user/attachment/system` + metadata rows; fields `uuid`, `parentUuid`, `sessionId`, `timestamp`, `cwd`, `gitBranch`, `version`, `isSidechain`, `promptId`, `requestId`; assistant rows embed `tool_use` blocks; user rows carry `tool_result` + top-level `toolUseResult` `{stdout, stderr, interrupted}`.
Also: `file-history/` (content-addressed pre-edit snapshots backing /rewind), `history.jsonl` (global prompt history), `shell-snapshots/`, `tasks/`.
**Format explicitly internal/unstable, drifts near-weekly**: defensive parsing, tolerate unknown types, never the sole pipeline. Prior art: simonw/claude-code-transcripts, Rust crate claude-code-transcripts.
Use for: backfill, tool outputs, and tamper evidence (transcript activity with no hook event = hooks disabled).

## OpenTelemetry (fallback)

`CLAUDE_CODE_ENABLE_TELEMETRY=1` + `OTEL_LOGS_EXPORTER=otlp` + endpoint; `OTEL_LOG_TOOL_DETAILS=1` unlocks tool_parameters (full bash_command, dangerouslyDisableSandbox flag!) and tool_input (truncated >512 chars). Events: user_prompt, tool_result (tool_use_id, decision_source), tool_decision, api_request, permission_mode_changed, mcp_server_connection. Tracon can embed an OTLP receiver; weakness: env must be set pre-launch, easy to unset. Offer for OTel-mandated orgs.

## Claude Desktop

- **No hooks in Desktop chat.** `claude_desktop_config.json` (`~/Library/Application Support/Claude/` mac, `%APPDATA%\Claude\` win) supports mcpServers only.
- **Code tab runs real Claude Code against the same ~/.claude settings + transcript dirs: our plugin fires there for free.**
- **Cowork**: isolated VM but tailable local stores under `.../Claude/local-agent-mode-sessions/**` incl. per-session `audit.jsonl`.
- Chat coverage: MCP logging proxy (rewrite mcpServers entries through a stdio gateway shim; pattern: https://github.com/P4ST4S/mcp-audit); ship our query server as a one-click `.mcpb` extension (https://github.com/modelcontextprotocol/mcpb). Built-in non-MCP Desktop tools are locally unobservable: state honestly in docs.

## Why not MCP for capture

MCP servers only see calls the model actively makes to their own tools; no subscription to all activity exists. MCP = query interface to the audit DB only.

## Prior art validating the design

- disler/claude-code-hooks-multi-agent-observability (hooks POST to localhost:4000, manual install; our plugin route is strictly cleaner)
- karanb192/claude-code-hooks (hooks distributed as a plugin marketplace)
- TechNickAI/claude_telemetry (PATH-shim to inject OTel env)
- Hook contract regressions to watch: anthropics/claude-code#49525 (payload contract), matcher semantics changes at v2.1.191/195/214. Read-only observer hooks are minimally exposed.
