# Tracon Research Archive

R&D conducted 2026-08-29 (six parallel research sweeps across two phases) before any code was written.

## Phase 1: should this exist, and on what?

1. [01-threat-landscape.md](01-threat-landscape.md) - supply-chain attacks 2018-2026, slopsquatting, auto-mode risk. Verdict: problem is real and growing.
2. [02-competitive-landscape.md](02-competitive-landscape.md) - IAXT, Dock Agent, Socket, Aikido, Endor et al. Verdict: gap at the intersection of OS truth + agent context + package intel + cross-platform.
3. [03-platform-selection.md](03-platform-selection.md) - Tauri 2.x + Rust core; OS monitoring APIs; distribution. Verdict: Tauri; integrate with agents rather than fight the OS for v1.

## Phase 2: how to integrate, how to run the project

4. [04-claude-integration.md](04-claude-integration.md) - Claude Code hooks/plugin/transcripts/OTel, Claude Desktop. Verdict: plugin + HTTP hooks primary.
5. [05-multi-agent-surfaces.md](05-multi-agent-surfaces.md) - Codex, Cursor, Gemini, Copilot, Windsurf, Aider adapters. Verdict: normalized event schema + thin adapters; Codex is integration #2.
6. [06-oss-standards.md](06-oss-standards.md) - AGPL-3.0 + CLA, ee/ open-core boundary, signed releases, naming. Verdict: open source day 1; name = Tracon.

## Rendered dossiers

- [dossiers/phase1-product-rd.html](dossiers/phase1-product-rd.html) (published: https://claude.ai/code/artifact/9619fcaa-6bda-40b5-9a87-72635ff36d5e)
- [dossiers/phase2-integration-rd.html](dossiers/phase2-integration-rd.html) (published: https://claude.ai/code/artifact/10c8589c-eacc-4a56-a62b-473695f9b7b7)
