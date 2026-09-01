# Open Source Standards + Business Model

Research date: 2026-08-29. Decision: AGPL-3.0 + CLA, open-core with future ee/ team tier, signed/attested releases from day 1.

## Why open source is mandatory here

The product's value proposition is "trust me to watch the watcher." Every comparable trust-critical endpoint tool is open: LuLu (GPL-3.0, marketed on "you are not trusting a black box"), Portmaster (AGPL/GPL), Bitwarden (GPL/AGPL), Tailscale (client fully OSS, control plane closed). A closed auditor would be rejected by exactly its target audience.

## License decision: AGPL-3.0 + CLA

- MIT/Apache: zero clone protection. Plausible started MIT, got cloned commercially, switched to AGPL. Wrong choice.
- GPL-3.0: fine for desktop but has the cloud loophole.
- **AGPL-3.0 (chosen)**: OSI-approved real open source; anyone running a modified version as a network service must publish source. Costs local desktop users nothing; bites only cloud cloners. Used by Plausible, Cal.com, Bitwarden server, Portmaster, Grafana.
- BSL/FSL/Elastic: source-available, not open source; "not actually open source" marketing tax outweighs the stronger non-compete for a trust product. FSL (Sentry) is the runner-up.
- **CLA (or DCO+CLA) from the first external PR** preserves dual-licensing rights: future paid team code lives in a clearly separated `ee/` under a commercial license (Cal.com pattern: "singleplayer = open source, multiplayer = commercial"; Bitwarden does the same).
- State the boundary in README from day 1: "The desktop recorder is and will remain free and AGPL. Paid features will only ever be team/server-side."

Sources: https://plausible.io/blog/open-source-licenses · https://cal.com/blog/changing-to-agplv3-and-introducing-enterprise-edition · https://github.com/bitwarden/server/blob/main/LICENSE_FAQ.md · https://fsl.software/

## Repo standards day 1

README (badges, screenshot, quickstart, threat-model paragraph), LICENSE, CONTRIBUTING.md, CODE_OF_CONDUCT.md (Contributor Covenant), SECURITY.md (GitHub Private Vulnerability Reporting, 90-day disclosure), issue/PR templates, Keep a Changelog + SemVer, Conventional Commits + release-please, branch protection, signed commits, OpenSSF Scorecard scheduled Action + Best Practices badge.

## Supply-chain pipeline (security tool = table stakes)

- GitHub Actions matrix (macos + windows) via tauri-action; tag-push releases.
- Apple Developer ID + notarization; Windows Azure Trusted Signing ($9.99/mo, open to individuals since Apr 2026).
- Tauri updater ed25519 signatures + GitHub Artifact Attestations / Sigstore cosign keyless (SLSA L2, L3 via reusable workflows).
- `cargo auditable` (dependency tree embedded in every binary) + CycloneDX SBOM release assets; `cargo deny` + `cargo audit` CI gates; Dependabot.
- Reproducible builds: Rust/Tauri not fully there (codegen nondeterminism, path embedding). Use trim-paths, SOURCE_DATE_EPOCH, codegen-units=1, and publish an honest "reproducibility status" page instead of claiming it.

## Repo layout (monorepo)

```
apps/desktop/        # Tauri 2.x app (src-tauri Rust + React/TS frontend)
crates/              # recorder core as reusable library crates
integrations/        # tracon-plugin (Claude Code), future agent adapters' installers
docs/                # Starlight site + this research
packaging/           # brew tap formula, winget manifest
.github/             # workflows, templates, CODEOWNERS, dependabot
ee/                  # (reserved, empty) future commercial team code
```

## Growth playbook

- Launch: Show HN with a concrete "here's what it caught" demo; r/ClaudeAI, r/LocalLLaMA; Product Hunt as echo only (~10% featured rate now).
- Positioning against cost dashboards: "what it DID, not what it cost" (the claude-code-otel/usage-monitor crowd measures cost; nobody owns the security/audit frame).
- Objective-See trust model: free core, no telemetry by default, "verify it yourself", published threat model, transparency page.
- Distribution day 1: GitHub Releases + own brew tap + winget + landing page with a "verify this download" section.
- Docs: Starlight (Astro) in-repo.

## Monetization boundary

Local/single-player = 100% free and open, forever: recording fidelity, timeline UI, local export, safety signals never paywalled. Paid = team sync server, org dashboard, SSO/SCIM, policy distribution, retention/compliance exports (SOC2 evidence packs). Market rates: Bitwarden $4-6/user/mo, dev-tool team tiers $4-10; target $8-10/dev/mo for compliance-flavored team tier.

## Naming

- "Blackbox" unusable (blackbox.ai, 30M-user AI coding product). "Flight Recorder" generic (JDK Flight Recorder etc.): descriptor, not brand.
- **Chosen: Tracon.** TRACON = FAA Terminal Radar Approach Control, the radar room tracking every aircraft in the airspace (= Tracon tracking every agent on your machine). Contains "trace". Availability checked 2026-08-29: npm 404 (free), crates.io 404 (free), tracon.dev no DNS record (likely unregistered). GitHub user "tracon" is taken by a Finnish convention org (non-software): use org `tracon-dev` or `traconhq`. Before public launch: register domain, USPTO TESS search Class 9/42.
