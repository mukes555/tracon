<div align="center">

<img src="docs/media/banner.png" width="760" alt="Tracon: a quokka in sunglasses holding a coffee mug, next to the wordmark and the tagline Flight recorder for AI coding agents" />

<br />

**A local recorder for what AI coding agents do on your machine.**<br />
Every command, file edit, and package install from Claude Code, Codex, Cursor, and Gemini CLI, with the dangerous ones flagged for review.

[![CI](https://github.com/mukes555/tracon/actions/workflows/ci.yml/badge.svg)](https://github.com/mukes555/tracon/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/mukes555/tracon?include_prereleases)](https://github.com/mukes555/tracon/releases)
[![License: AGPL-3.0](https://img.shields.io/badge/license-AGPL--3.0-blue)](LICENSE)
![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows-lightgrey)
[![Built with Tauri](https://img.shields.io/badge/built%20with-Tauri%202-24C8DB)](https://tauri.app)

<br />

<img src="docs/media/demo.gif" width="860" alt="A tour of Tracon: the Overview, the Live page with three agents streaming, a session timeline with a flagged command open in the inspector, the conversation behind it, the Flagged inbox grouped by severity, the package ledger, Settings, and the command palette" />

</div>

## Why

Agents run in auto-accept mode all day. They install packages, run shell commands, and rewrite files, and almost none of it gets reviewed. Tracon records what they actually did so you can check afterwards.

## The screens

**Overview** is the day at a glance: agents working right now, today's counts, a 14 day activity chart, and a flag inbox you can clear without leaving the page.

<img src="docs/media/overview.png" width="860" alt="The Overview page: three live sessions, today's counts, an activity chart, the flag inbox, and recent packages" />

**Live** gives each active session its own monitor, streaming as the agent works: the task it was given, the subagents it spawned, and its last few actions, with flagged commands lit in red.

<img src="docs/media/live.png" width="860" alt="The Live page: session monitors with dark terminal panes streaming recent commands" />

**Timeline** is the full ledger. Sessions on the left, and the selected session's prompts, commands, file edits, and installs in order on the right. Click a row and the inspector shows the command, why it was flagged, and the raw payload.

<img src="docs/media/timeline.png" width="860" alt="The Timeline page: a session list, the event ledger, and the inspector explaining a pipe to shell flag" />

**Flagged** is an inbox, not a graveyard. Open flags group by severity, critical first. Acknowledge one or a whole group, with undo.

<img src="docs/media/flagged.png" width="860" alt="The Flagged page: open flags grouped into Critical and Warning with acknowledge buttons" />

Also: **Packages**, every install with an optional threat intelligence check. **Conversation**, the chat behind any Claude Code or Codex event, read from the agent's own transcript. **Search**, a `Cmd+K` palette over everything recorded.

### Keyboard

| Keys | Action |
| --- | --- |
| `Cmd+K` | Search everything, or type `>` for commands |
| `1` to `6` | Switch pages |
| `Up` `Down` or `j` `k` | Move through rows |
| `Enter` | Open the selected row |
| `A` | Acknowledge the open flag and move to the next |
| `Esc` | Close the inspector |

**Simple** and **Advanced** in the top bar hide or show operator details: event source, session id, and the raw payload.

## What gets flagged

Recursive deletes, remote scripts piped to a shell, credential file access, force pushes, agents started with permission prompts disabled, world-writable permissions, and raw disk operations. With threat intelligence on, packages with a published advisory or a version published in the last 48 hours.

Flags are advisory. Tracon records and warns; it never blocks or slows an agent, and a closed or crashed Tracon changes nothing about how they run.

## Capture

| Agent | Mechanism | Setup |
| --- | --- | --- |
| Claude Code | HTTP hooks, real time | The [Tracon plugin](integrations/claude-plugin) |
| Claude Code | Transcript tailing | Automatic, read-only, `~/.claude/projects` |
| Codex CLI | Rollout tailing | Automatic, `~/.codex/sessions` |
| Cursor | Hooks | Snippet for `~/.cursor/hooks.json`, shown in Settings |
| Gemini CLI | Hooks | See [integrations/gemini-hooks](integrations/gemini-hooks) |

Tracon never writes to agent directories and never edits agent configuration. Tailing is read-only and the capture offsets live in Tracon's own database.

Hooks authenticate with a per-install token written to `~/.tracon/token` on first launch, and the ingest server binds only to `127.0.0.1:48620`, so other local processes and web pages cannot forge events or hide real ones.

## Install

**macOS, with Homebrew:**

```bash
brew tap mukes555/tap
brew install --cask tracon
```

The cask picks the Apple Silicon or Intel build and clears the Gatekeeper quarantine, since releases are not yet code signed. `brew upgrade --cask tracon` picks up new versions.

**Or download** a macOS DMG or a Windows installer from [Releases](https://github.com/mukes555/tracon/releases). On macOS, right click the app and choose Open the first time; Windows SmartScreen may ask you to confirm once. Every installer carries a Sigstore provenance attestation:

```bash
gh attestation verify Tracon_0.4.0_aarch64.dmg --repo mukes555/tracon
```

**Or build from source.** Needs [Rust](https://rustup.rs), Node 22+, [pnpm](https://pnpm.io), and the [Tauri prerequisites](https://tauri.app/start/prerequisites/):

```bash
git clone https://github.com/mukes555/tracon
cd tracon/apps/desktop
pnpm install
pnpm tauri dev
```

For real-time Claude Code capture, add the plugin from GitHub with no clone:

```
/plugin marketplace add mukes555/tracon
/plugin install tracon@tracon
```

## Privacy

Your audit data never leaves your machine, and there is no telemetry. Two features touch the network, both off by default and both switchable in Settings:

- **Threat intelligence** sends package names, and nothing else, to osv.dev and registry.npmjs.org.
- **The update check** asks GitHub for the latest release number. A Check now button works without turning the daily check on.

History is kept for 90 days by default. Settings can change that, delete a single session, or delete everything.

## Architecture

A Tauri 2 desktop app: a Rust core and a React and TypeScript interface over a local SQLite database.

```
crates/tracon-core      event model and SQLite store
crates/tracon-adapters  per-agent normalizers and the danger heuristics
crates/tracon-ingest    hook server, log tailers, spool, background workers
apps/desktop            Tauri shell and the interface
integrations/           hook configs and the Claude Code plugin
```

Events arrive through hooks or read-only tailing, are normalized to one shape, deduplicated, and stored locally. The interface polls a small change token and reads through a separate connection pool, so capture never blocks the window.

## Built by AI agents

<img src="docs/media/quokka-live.png" width="320" align="right" alt="The Tracon quokka at a desk in front of monitors full of code" />

Tracon is written largely by AI coding agents, with human direction and review. A tool that audits agents should say so. Every change passes the same gate before it lands: `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and a typechecked frontend build with its tests, all run by the CI in this repository.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) and the [good first issues](https://github.com/mukes555/tracon/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22). AI-assisted contributions are welcome; review every line yourself and say so in the pull request.

## License

[AGPL-3.0](LICENSE). Free for individual use.
