# Tracon hooks for Gemini CLI

Streams Gemini CLI agent activity (tool calls, shell commands, file writes, session lifecycle) to the Tracon desktop app over localhost. Observer only: the command prints nothing and exits 0, so it never blocks or alters anything Gemini does.

## Install (manual, by you)

Tracon never edits your configs. Merge this into your `~/.gemini/settings.json` (Gemini reads hooks from there):

```json
{
  "hooks": {
    "SessionStart": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "sh -c 'curl -s -m 5 -X POST -H \"content-type: application/json\" -H \"X-Tracon-Token: $(cat \"$HOME/.tracon/token\" 2>/dev/null)\" --data-binary @- http://localhost:48620/ingest/gemini -o /dev/null; exit 0'",
            "timeout": 5000
          }
        ]
      }
    ],
    "SessionEnd": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "sh -c 'curl -s -m 5 -X POST -H \"content-type: application/json\" -H \"X-Tracon-Token: $(cat \"$HOME/.tracon/token\" 2>/dev/null)\" --data-binary @- http://localhost:48620/ingest/gemini -o /dev/null; exit 0'",
            "timeout": 5000
          }
        ]
      }
    ],
    "BeforeTool": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "sh -c 'curl -s -m 5 -X POST -H \"content-type: application/json\" -H \"X-Tracon-Token: $(cat \"$HOME/.tracon/token\" 2>/dev/null)\" --data-binary @- http://localhost:48620/ingest/gemini -o /dev/null; exit 0'",
            "timeout": 5000
          }
        ]
      }
    ],
    "AfterTool": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "sh -c 'curl -s -m 5 -X POST -H \"content-type: application/json\" -H \"X-Tracon-Token: $(cat \"$HOME/.tracon/token\" 2>/dev/null)\" --data-binary @- http://localhost:48620/ingest/gemini -o /dev/null; exit 0'",
            "timeout": 5000
          }
        ]
      }
    ]
  }
}
```

Note the dedicated `/ingest/gemini` endpoint: Gemini payloads look like Claude Code's, so Tracon routes them explicitly rather than guessing.

## The token

Anything on your machine, including a web page doing a cross-origin POST, can reach `localhost:48620`, so Tracon only accepts events that present the secret in `~/.tracon/token` (created by the app, readable only by you). The hook command reads that file with `$(cat ~/.tracon/token)` and sends it as the `X-Tracon-Token` header; without it the app answers 403 and records nothing.

Windows without Git Bash: replace the `sh -c '...'` command with a PowerShell equivalent, for example
`powershell -NoProfile -Command "$t = Get-Content $env:USERPROFILE\.tracon\token; $b = [Console]::In.ReadToEnd(); Invoke-RestMethod -Method Post -Uri http://localhost:48620/ingest/gemini -ContentType application/json -Headers @{'X-Tracon-Token'=$t} -Body $b | Out-Null; exit 0"`.

## Privacy

Events go to localhost and nowhere else. BeforeModel/AfterModel request-level events are intentionally not wired: too noisy for an audit trail.
