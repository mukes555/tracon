# Tracon Roadmap

Updated 2026-09-01.

## Where we are

Working three-and-a-half-agent recorder, verified end to end on real data:

- Claude Code: plugin + HTTP hooks (live-tested with v2.1.235), transcript tailing, spool backfill, cross-source dedupe proven.
- Codex CLI: rollout tailing with sandbox/approval context.
- Cursor: hooks adapter + manual-install integration.
- Gemini CLI: hooks adapter on a dedicated ingest route.
- Danger flags, package detection, opt-in OSV/freshness intel, retention purge (90d default), timeline/packages/flagged UI with search, filters, drill-down, JSON export, capture status panel.
- AGPL-3.0, CI (fmt/clippy/tests on mac+win), tag-triggered release workflow, packaging scaffolds. 37+ tests.

## v0.1.0 - first public release

Dev tasks (Claude can do):
- [ ] App icon (replace the Tauri default; tray needs a template icon on macOS)
- [ ] Settings view in-app (port, retention days, intel toggle relocation)
- [ ] Onboarding first-run screen (detect agents, copy-paste install snippets)
- [ ] README screenshots + quickstart GIF
- [ ] Windows smoke test (CI builds it; needs one manual run)

Owner tasks (only the user can do):
- [ ] Create GitHub org (tracon-dev or traconhq) + repo, push, enable Private Vulnerability Reporting
- [ ] Register tracon.dev
- [ ] git tag v0.1.0 -> CI builds draft release -> publish
- [ ] Apple Developer Program ($99/yr) for signing/notarization; Azure Trusted Signing ($9.99/mo) for Windows
- [ ] Fill brew tap + winget manifests from the release artifacts

## v0.2 - trust and depth

- Copilot CLI adapter (hooks + per-session events.jsonl)
- Tamper evidence surfaced in UI: transcript activity with no matching hook stream = "hooks were disabled" banner
- Divergence detection groundwork: process-poll attribution (unprivileged) tying installs to agent process trees
- OpenSSF Scorecard + Best Practices badge, cargo-auditable + SBOM in release workflow
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
