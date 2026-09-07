# Tracon plugin for Claude Code

Streams every Claude Code hook event to the Tracon desktop app over localhost. Pure observer: every hook is a short-timeout `curl` that always exits 0, so a closed or crashed Tracon never slows or blocks the agent.

## What it captures

Session lifecycle, prompts, every tool call and result (including the full Bash command line and edited file paths), permission requests/denials, subagent lifecycle, compaction, and settings changes (a tamper signal).

Tool calls are additionally appended to a local spool file (`~/.tracon/spool.ndjson`) by an async command hook, so commands run while the Tracon app is closed are backfilled the next time it starts. Each spooled line carries the time the hook fired, so backfilled events keep their real timestamp. The spool stops growing at 50 MB until the app drains it.

## The token

Anything on your machine, including a web page doing a cross-origin POST, can reach `localhost:48620`, so Tracon only accepts events that present the secret in `~/.tracon/token` (created by the app, readable only by you). The hooks read that file and send it as the `X-Tracon-Token` header; without it the app answers 403 and records nothing.

The hooks use `sh` and `curl`, which Claude Code already requires (Git Bash on Windows). If you would rather use Claude Code's native HTTP hooks, they can carry the header from an environment variable instead of the file:

```json
{ "type": "http", "url": "http://localhost:48620/ingest",
  "headers": { "X-Tracon-Token": "$TRACON_TOKEN" }, "allowedEnvVars": ["TRACON_TOKEN"], "timeout": 5 }
```

with `TRACON_TOKEN` exported from the file (PowerShell: `$env:TRACON_TOKEN = Get-Content $env:USERPROFILE\.tracon\token`).

## Files on disk

`~/.tracon/` holds only the spool file and the token. Uninstalling the plugin and the app should remove that directory; nothing else is written outside Tracon's own data directory.

## Install (development)

From this repository:

```
claude --plugin-dir ./integrations/claude-plugin
```

Or add the marketplace once the repo is on GitHub:

```
/plugin marketplace add tracon-dev/tracon
/plugin install tracon@tracon
```

## Privacy

Events go to `http://localhost:48620/ingest` and nowhere else. Tracon stores them in a local SQLite database. Nothing leaves your machine.

## Known behavior

SessionStart can fire before plugin hooks finish registering, so a session
captured via the plugin may lack an explicit session_start event. Harmless:
Tracon derives sessions from their first observed event, and the transcript
tailer fills historical context.
