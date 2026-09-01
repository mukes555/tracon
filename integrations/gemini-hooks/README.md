# Tracon hooks for Gemini CLI

Streams Gemini CLI agent activity (tool calls, shell commands, file writes, session lifecycle) to the Tracon desktop app over localhost. Observer only: the command prints nothing and exits 0, so it never blocks or alters anything Gemini does.

## Install (manual, by you)

Tracon never edits your configs. Merge this into your `~/.gemini/settings.json` (Gemini reads hooks from there):

```json
{
  "hooks": {
    "SessionStart": [
      { "hooks": [{ "type": "command", "command": "sh -c 'cat | curl -s -m 5 -X POST -H \"content-type: application/json\" --data-binary @- http://localhost:48620/ingest/gemini -o /dev/null; exit 0'", "timeout": 5000 }] }
    ],
    "SessionEnd": [
      { "hooks": [{ "type": "command", "command": "sh -c 'cat | curl -s -m 5 -X POST -H \"content-type: application/json\" --data-binary @- http://localhost:48620/ingest/gemini -o /dev/null; exit 0'", "timeout": 5000 }] }
    ],
    "BeforeTool": [
      { "hooks": [{ "type": "command", "command": "sh -c 'cat | curl -s -m 5 -X POST -H \"content-type: application/json\" --data-binary @- http://localhost:48620/ingest/gemini -o /dev/null; exit 0'", "timeout": 5000 }] }
    ],
    "AfterTool": [
      { "hooks": [{ "type": "command", "command": "sh -c 'cat | curl -s -m 5 -X POST -H \"content-type: application/json\" --data-binary @- http://localhost:48620/ingest/gemini -o /dev/null; exit 0'", "timeout": 5000 }] }
    ]
  }
}
```

Note the dedicated `/ingest/gemini` endpoint: Gemini payloads look like Claude Code's, so Tracon routes them explicitly rather than guessing.

## Privacy

Events go to localhost and nowhere else. BeforeModel/AfterModel request-level events are intentionally not wired: too noisy for an audit trail.
