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

Releases are built in CI from tagged commits by the workflow in this repository, and every installer carries a Sigstore build provenance attestation. Verify one with:

```bash
gh attestation verify Tracon_0.3.0_aarch64.dmg --repo mukes555/tracon
```

Builds are not yet code signed or notarized (that needs a paid Apple Developer account and a Windows signing certificate; both are on the roadmap). Until then, the Homebrew cask verifies the download checksum and the attestation above ties the file to the commit that built it. SBOMs and cargo auditable are not yet produced.

## Local threat model

The ingest server binds only to 127.0.0.1 and requires a per-install token (`~/.tracon/token`, mode 0600) on every POST, so other local processes and web pages cannot forge or suppress events. Transcript tailing is read-only, never follows symlinks, and never writes to agent directories. Recorded agent output is stored as data and rendered as text; it is never executed.
