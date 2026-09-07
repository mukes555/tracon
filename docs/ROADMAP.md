# Tracon Roadmap

Updated 2026-09-08.

## Where we are (v0.3.0 shipped)

- Claude Code: plugin + HTTP hooks, transcript tailing, spool backfill, cross-source dedupe proven.
- Codex CLI: rollout tailing. Cursor and Gemini CLI: hooks adapters.
- Danger flags, package detection, opt-in OSV/freshness intel, retention purge (90d default).
- Overview, Live, Timeline, Packages, Flagged, Settings; docked inspector; keyboard triage; command palette.
- Quokka branding, app icon, menu bar icon. README tour with sample-data screenshots.
- AGPL-3.0, CI (fmt, clippy, tests, frontend build), tag-triggered releases with provenance attestations, Homebrew cask.

## Next (v0.4)

Dev tasks:
- [ ] Ingest token, sanitized session ids, panic-proof tailers, bounded spool (in progress)
- [ ] Local-time "today", poll error states, honest ack and settings feedback (in progress)
- [ ] Session rollup table so stats and sessions stop scanning the events table (in progress)
- [ ] Capture-paused banner and switch, delete data controls, version and log file in Settings (in progress)
- [ ] Frontend tests, Linux CI, dependency audits, pinned toolchain (in progress)
- [ ] Copilot CLI adapter (hooks + per-session events.jsonl)
- [ ] Onboarding first-run screen (detect agents, copy-paste install snippets)
- [ ] Ship the Claude plugin inside the app bundle so brew users get real-time hooks without a clone
- [ ] Windows smoke test of the release build

Owner tasks (only the user can do):
- [ ] Add the HOMEBREW_TAP_TOKEN secret so the tap updates itself on release
- [ ] Apple Developer Program for signing and notarization; Azure Trusted Signing for Windows
- [ ] Register tracon.dev; enable Private Vulnerability Reporting

## v0.2 - trust and depth

- Copilot CLI adapter (hooks + per-session events.jsonl)
- Tamper evidence surfaced in UI: transcript activity with no matching hook stream = "hooks were disabled" banner
- Divergence detection groundwork: process-poll attribution (unprivileged) tying installs to agent process trees
- Auto-update (Tauri updater) once builds are signed; OpenSSF Scorecard; cargo-auditable + SBOM in the release workflow
- Docs site (Starlight) with threat model page ("what Tracon can and cannot see")

## v0.3 - deep mode (opt-in elevation)

- Windows: ETW kernel-process trace via elevated helper service
- macOS: eslogger under a privileged helper (root + Full Disk Access)
- Apply for the Apple Endpoint Security entitlement (multi-month lead time; start early)
- Divergence alerts: OS-level events with no agent-log counterpart

## v0.4 - team tier groundwork (the ee/ boundary)

- Local app stays 100% free/AGPL forever (recording fidelity and safety signals never paywalled)
- Paid: team sync server, org dashboard, SSO, policy distribution (managed-settings recipes), compliance exports
- Target $8-10/dev/mo per the market research

## Launch playbook (from research)

1. Show HN: "Tracon - a flight recorder for AI coding agents", with a real caught-in-the-act demo (the e2e session + a flagged rm -rf)
2. r/ClaudeAI and r/LocalLLaMA posts; Product Hunt as echo only
3. Positioning: "what your agents DID, not what they cost" (vs the cost-dashboard crowd); "the auditor you can audit" (vs closed competitors: IAXT is macOS-only, Dock Agent is Claude-only)
4. Transparency page: threat model, no-telemetry proof, how to verify builds
