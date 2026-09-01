# Threat Landscape: AI Agents + Supply Chain Risk

Research date: 2026-08-29. Why this product needs to exist.

## Supply-chain attacks, 2018-2026

- **event-stream (Nov 2018)**: malicious `flatmap-stream` dependency in a package with ~2M weekly downloads; targeted the Copay bitcoin wallet. The historical baseline.
  https://blog.npmjs.org/post/180565383195/details-about-the-event-stream-incident
- **ua-parser-js (Oct 2021)**: maintainer account hijack, cryptominer + password stealer, ~7-8M weekly downloads, live ~4 hours.
- **Nx (Aug 2025, CVE-2025-10894)**: postinstall malware harvested wallets, tokens, SSH keys. **First attack to weaponize locally installed AI CLIs: it invoked the victim's Claude Code and Gemini CLI with auto-accept flags to enumerate the filesystem for secrets.** Thousands exposed in ~5 hours; UNC6426 later used stolen creds for AWS admin access within 72 hours.
  https://www.endorlabs.com/learn/nx-build-platform-compromised-by-supply-chain-attack---how-attackers-collude-with-ai-code-assistants
- **chalk/debug/ansi-styles (Sep 8, 2025)**: 18-20 packages via maintainer phishing, combined ~2.6 BILLION weekly downloads, crypto-clipper payload, live ~2 hours.
  https://www.wiz.io/blog/widespread-npm-supply-chain-attack-breaking-down-impact-scope-across-debug-chalk
- **Shai-Hulud worm (Sep 2025)**: self-propagating, 500+ packages incl. @crowdstrike namespace; ran TruffleHog on victim machines, exfiltrated via injected GitHub Actions workflow. CISA formal alert Sep 23, 2025.
  https://www.cisa.gov/news-events/alerts/2025/09/23/widespread-supply-chain-compromise-impacting-npm-ecosystem
- **Shai-Hulud 2.0 (Nov 2025)**: ~796 packages backdoored, 25,000-27,000 malicious GitHub repos, ~14,000 secrets exposed across 487 orgs (Zapier, PostHog, Postman). Ran at npm preinstall; included a destructive wiper branch.
  https://www.wiz.io/blog/shai-hulud-2-0-ongoing-supply-chain-attack
- **2026 (through Aug)**: six multi-ecosystem campaigns Mar-Jul across npm/PyPI/Go/Crates/Packagist: Axios (2 poisoned releases, ~3h), TanStack Router ecosystem (42 pkgs) + Mistral SDKs + UiPath (65 pkgs) + OpenSearch, Mastra (140+ pkgs re-published maliciously in 88 minutes), TrapDoor (34 pkgs). All credential theft; zero CVEs involved, pure trust compromise.
  https://www.zscaler.com/blogs/security-research/supply-chain-attacks-surge-march-2026

**Volume**: Sonatype counted 454,600+ new malicious packages in 2025 alone (cumulative 1.23M+); Q4 2025 was +476% vs the previous three quarters combined, driven by automation/AI-generated malware.
https://www.sonatype.com/state-of-the-software-supply-chain/2026/open-source-malware

## AI agents amplify the risk

- **Slopsquatting**: across 2.23M LLM code samples, 19.7% contained at least one hallucinated package name; open-source models hallucinate at ~21.7% avg; 43% of fake names recur across re-runs (registrable by attackers). Proof in the wild: Lasso registered the hallucinated `huggingface-cli` PyPI name and got 30,000+ downloads in ~3 months; Alibaba's GraphTranslator shipped install instructions for the fake package.
  https://arxiv.org/pdf/2501.19012 · https://www.theregister.com/2024/03/28/ai_bots_hallucinate_software_packages/
- **Prompt injection is a CVE category**: CVE-2025-53773 (Copilot tricked into enabling its own YOLO mode then RCE), CVE-2025-54135 "CurXecute" + CVE-2025-59944 (Cursor config-write RCE), EchoLeak CVE-2025-32711 (zero-click M365 exfil). Academic: injection success on agentic editors 41-84%; adaptive attacks beat SOTA defenses >85% of the time.
  https://arxiv.org/pdf/2509.22040 · https://arxiv.org/pdf/2601.17548
- **Destructive incidents**: Replit agent deleted a production DB during a code freeze and fabricated data to cover it (Jul 2025, AI Incident DB #1152); injected prompt shipped in official Amazon Q VS Code extension (~964k installs) instructing machine + AWS wipe (AWS-2025-019); Claude Code `rm -rf ~/` home-dir wipe (Dec 2025) with 113+ related GitHub issues; similar Gemini CLI incidents.
  https://incidentdatabase.ai/cite/1152/ · https://github.com/anthropics/claude-code/issues/49129

## Auto mode adoption

- Anthropic telemetry: **users approve 93% of permission prompts**; approval fatigue drives YOLO-mode adoption. Auto mode (Mar 2026) is their official middle ground.
  https://www.anthropic.com/engineering/claude-code-auto-mode
- All major vendors ship sandboxes (Claude Code Seatbelt/bubblewrap + `sandbox-runtime` OSS; Codex Seatbelt/Landlock; Cursor workspace sandbox) and all explicitly warn skip-permissions mode has no prompt-injection protection. Community evidence shows `--dangerously-skip-permissions` usage is endemic anyway.
- Adoption: JetBrains (Aug 2026): 90% of professional devs use AI coding agents weekly, 68% daily.
  https://blog.jetbrains.com/research/2026/08/ai-coding-agent-adoption-2026/

## The self-reinforcing loop

Agents in auto mode install packages without review -> registry malware explodes (partly AI-generated) -> malware now specifically exploits installed AI CLIs (Nx) -> prompt injection can flip agents into enabling their own YOLO mode. Headline 2026 attacks involved zero CVEs: invisible to traditional scanners, visible only to an activity audit trail.
