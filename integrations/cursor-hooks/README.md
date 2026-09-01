# Tracon hooks for Cursor

Streams Cursor agent activity (shell commands, file edits, MCP calls, prompts, session lifecycle) to the Tracon desktop app over localhost. Same posture as the Claude Code plugin: pure observer. The hook command prints nothing and always exits 0, so it never allows, denies, or delays anything; Cursor's hooks are fail-open, and a closed Tracon just means the POST silently no-ops.

## Install (manual, by you)

Tracon never edits your configs. Merge the `hooks` entries from [hooks.json](hooks.json) into your own `~/.cursor/hooks.json` (create it if missing), then restart Cursor: it reads hook config only at startup. Project-scoped alternative: `.cursor/hooks.json` in a repo.

## Privacy

Events go to `http://localhost:48620/ingest` and nowhere else.

## Notes

- Requires `curl` (present on macOS and modern Windows).
- Cursor has no per-tool-call id, so Tracon dedupes on conversation + generation + command content.
- This covers the IDE agent and the `cursor-agent` CLI (both fire the same hooks).
