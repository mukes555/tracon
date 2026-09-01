# Tracon plugin for Claude Code

Streams every Claude Code hook event to the Tracon desktop app over localhost. Pure observer: every HTTP hook has a short timeout and Claude Code treats endpoint failures as non-blocking, so a closed or crashed Tracon never slows or blocks the agent.

## What it captures

Session lifecycle, prompts, every tool call and result (including the full Bash command line and edited file paths), permission requests/denials, subagent lifecycle, compaction, and settings changes (a tamper signal).

Tool calls are additionally appended to a local spool file (`~/.tracon/spool.ndjson`) by an async command hook, so commands run while the Tracon app is closed are backfilled the next time it starts. Spooled events get their timestamp at drain time, not execution time; the transcript tailer corrects this in a later milestone.

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

## Windows note

The spool command hook uses `sh`; on Windows it requires Git Bash on PATH. The HTTP hooks (the primary path) work everywhere.
