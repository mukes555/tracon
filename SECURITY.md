# Security Policy

Tracon is a security tool: it records what AI coding agents do on your machine. We hold it to the standard we ask of the agents.

## Reporting a vulnerability

Please report vulnerabilities privately via GitHub Private Vulnerability Reporting on this repository (Security tab, "Report a vulnerability"). Do not open public issues for security reports.

We aim to acknowledge reports within 72 hours and to ship a fix or mitigation within 90 days of triage. We will credit reporters in the release notes unless you ask otherwise.

## Scope

In scope: the desktop app, the ingest server, the agent plugins/adapters, and the release pipeline (a compromised release is a vulnerability).

Notable design guarantees worth attacking:
- Audit data must never leave the machine (no telemetry by default).
- The ingest server must only ever bind to localhost.
- A malformed or hostile hook payload must never crash the recorder or corrupt the store.
- Recorded agent output is untrusted data and must never be executed or interpreted.

## Supply chain

Releases are built in CI from tagged commits, signed, and published with SBOMs. Binaries are built with cargo auditable so their dependency tree is embedded and verifiable.
