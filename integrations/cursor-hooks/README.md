# Tracon hooks for Cursor

Streams Cursor agent activity (shell commands, file edits, MCP calls, prompts, session lifecycle) to the Tracon desktop app over localhost. Same posture as the Claude Code plugin: pure observer. The hook command prints nothing and always exits 0, so it never allows, denies, or delays anything; Cursor's hooks are fail-open, and a closed Tracon just means the POST silently no-ops.

## Install (manual, by you)

Tracon never edits your configs. Merge the `hooks` entries from [hooks.json](hooks.json) into your own `~/.cursor/hooks.json` (create it if missing), then restart Cursor: it reads hook config only at startup. Project-scoped alternative: `.cursor/hooks.json` in a repo.

## The token

Anything on your machine, including a web page doing a cross-origin POST, can reach `localhost:48620`, so Tracon only accepts events that present the secret in `~/.tracon/token` (created by the app, readable only by you). The hook command reads that file with `$(cat ~/.tracon/token)` and sends it as the `X-Tracon-Token` header; without it the app answers 403 and records nothing.

Windows without Git Bash: replace the `sh -c '...'` command with a PowerShell equivalent, for example
`powershell -NoProfile -Command "$t = Get-Content $env:USERPROFILE\.tracon\token; $b = [Console]::In.ReadToEnd(); Invoke-RestMethod -Method Post -Uri http://localhost:48620/ingest -ContentType application/json -Headers @{'X-Tracon-Token'=$t} -Body $b | Out-Null; exit 0"`.

## Privacy

Events go to `http://localhost:48620/ingest` and nowhere else.

## Notes

- Requires `curl` (present on macOS and modern Windows).
- Cursor has no per-tool-call id, so Tracon dedupes on conversation + generation + command content.
- This covers the IDE agent and the `cursor-agent` CLI (both fire the same hooks).
