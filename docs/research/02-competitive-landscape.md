# Competitive Landscape

Research date: 2026-08-29. Who exists, and where the gap is.

## Direct-adjacent desktop products

- **IAXT** (https://iaxt.com) - THE closest competitor. macOS menu-bar app: local audit trail of agent commands, file changes, package installs, git ops, persistence, network. Passive user-space monitoring (FSEvents/kqueue/sysctl), triages events (Routine/Flagged/Review), SQLite local. Supports Claude Code, Cursor, Aider, Codex, Windsurf, Copilot, Cody +. Free individual tier; team tier in private beta. **macOS only, no Windows.**
- **Dock Agent** (https://dockagent.app) - floating overlay for Claude Code sessions: live timeline, pre-execution approval, danger-pattern highlighting. Mac + Win, $14.99 one-time. **Claude Code only.**
- **AgentsView** (https://www.agentsview.io) - OSS local session browser parsing JSONL of 20+ agents; analytics/cost dashboards. **Self-reported data only, no OS ground truth, no security framing.**
- hoangsonww/Claude-Code-Agent-Monitor - OSS realtime dashboard for Claude Code and Codex sessions.

## Package-install-time security (no UI, no agent context)

- **Socket Firewall (sfw)** - local proxy wrapper for npm/yarn/pnpm/pip/uv/cargo; free tier; platform $25-50/dev/mo. CLI/CI only. https://docs.socket.dev/docs/socket-firewall-overview
- **Aikido Safe Chain** - free OSS local proxy across 10 package managers, threat feed, 48h min-age gate. Great integration candidate as a threat feed. https://github.com/AikidoSec/safe-chain
- **npq** (lirantal) - pre-install package audit prompts, npm only. https://github.com/lirantal/npq
- Phylum acquired by Veracode (Jan 2025); Datadog GuardDog (OSS scanner, CI); Snyk (CVE-centric, weak on live malicious-package blocking); npm audit (post-install, noisy, too late by design).

## Enterprise AI-agent governance (SaaS, CISO buyer)

- **Endor Labs agent governance** (May 2026): agent/MCP/skill inventory + policy across workstations + package firewall. Closest enterprise analogue; watch for down-market moves. https://www.endorlabs.com/ai-coding-agent-governance
- **Lasso Security**, **Prompt Security** (acquired by SentinelOne, ~$159-250M, Sep 2025; MCP Gateway), **Coder Agent Firewall** (only inside Coder workspaces), **Pipelock** (OSS agent-internet firewall, May 2026).

## Sandboxing (prevention, not audit; complementary)

- Claude Code `/sandbox` (Seatbelt/bubblewrap + network allowlist proxy) + OSS `anthropic-experimental/sandbox-runtime`.
- Docker Sandboxes (`sbx`): per-agent microVMs for Claude Code/Codex/Gemini/Copilot.
- OSS Seatbelt wrappers (agent-seatbelt-sandbox, sbx, Agent Safehouse); cloud sandboxes (E2B, Daytona, Modal).

## Desktop security apps (UX references)

Little Snitch (~EUR 59, the canonical alert UX), LuLu + BlockBlock (Objective-See, free/OSS), Santa (binary authorization via Endpoint Security), GlassWire and Portmaster (Windows firewalls).

## Gap analysis

No product combines all four:
1. OS-level ground truth (process tree, file events)
2. Agent session context (which agent, which session, which prompt)
3. Package-install intelligence (what was installed + threat-feed flags)
4. Mac AND Windows

IAXT does 1-3 on one platform. Tracon's wedge: attribution ("Claude Code session X ran npm install pulling 38 transitive packages, one flagged") + cross-platform + open source.

Strategic risks: Anthropic first-party observability convergence (hooks + OTel + sandbox); IAXT shipping Windows; Endor down-market. Defense: multi-agent cross-vendor local-first position (no agent vendor will audit its competitors), open source trust.
