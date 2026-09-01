# Platform Selection: Tauri 2.x + Rust Core

Research date: 2026-08-29. Decision: Tauri 2.x, Rust monitoring core, React/TS frontend.

## Framework comparison (2025-2026)

| Framework | Bundle | Idle RAM | Tray/menubar | Verdict |
|---|---|---|---|---|
| Tauri 2.x | 3-10 MB | ~30-50 MB | first-class | **chosen** |
| Electron | 80-165 MB | 150-300 MB | mature | viable, heavy for an all-day background app |
| Flutter desktop | 20-40 MB | 100 MB+ | weak third-party | poor fit |
| .NET MAUI | large | moderate | macOS story weakest | poor fit |
| Swift + WinUI pair | small | lowest | perfect | 2x codebases, only for ES-entitled EDR-grade product |
| Wails v3 (Go) | ~10 MB | low | beta | Go lacks Endpoint Security bindings |

Precedents: 1Password 8 = Rust core + web shell; Raycast v2 = mixed with Rust indexer; Warp = Rust; Claude Desktop/Cursor/Slack/Linear = Electron. Hoppscotch's Electron->Tauri migration: 165 MB -> 8 MB, ~70% RAM cut.

Why Rust core is forced anyway: the only language with credible bindings for all three deep OS paths:
- `notify` v6+ (FSEvents / ReadDirectoryChangesW / inotify unified)
- `ferrisetw` (Windows ETW consumption)
- `endpoint-sec` (macOS Endpoint Security, for when the entitlement lands)
- plus `windows-rs`, `sysinfo`, `rusqlite`

With Tauri that core IS the app; with Electron it would be a NAPI sidecar (same work, extra seam).

## Monitoring layer per OS

### macOS
- **Endpoint Security framework**: the "correct" API but requires the restricted `com.apple.developer.endpoint-security.client` entitlement, granted by application only; reported waits 4+ months, some devs denied distribution. Do NOT gate MVP on it; apply in parallel.
- **eslogger** (macOS 13+): Apple-shipped CLI exposing all ES notify events as JSON with only root + Full Disk Access. Pragmatic opt-in "deep mode" backdoor (schema officially unstable).
- **FSEvents**: userland, zero privileges, the default file layer.
- **Process attribution without entitlements**: poll sysctl KERN_PROC_ALL / libproc at 1-2s (cheap), KERN_PROCARGS2 for argv, walk PPIDs to attribute activity to agent process trees. kqueue EVFILT_PROC for exit events of known PIDs.
- DTrace needs SIP off (non-starter). NetworkExtension content filter (Little Snitch's API) is self-service entitlement but a system extension: v3 territory.

### Windows
- **ETW**: kernel process/file/network providers, userland consumption, NO signed driver needed. Requires admin or Performance Log Users membership: run an elevated/LocalService helper for the trace session, unelevated UI. Avoid minifilter drivers entirely.
- No-privilege fallback: WMI polled intrinsic events (__InstanceCreationEvent WITHIN n) or Toolhelp snapshots; ReadDirectoryChangesW for files.
- Sysmon: integrate if present (read its event log), never depend on it.

## Distribution

- **Mac App Store: not feasible** (sandbox denies observing other processes and ~/.claude). Direct: Apple Developer Program $99/yr, Developer ID + notarytool (automated in CI, Tauri built-in).
- **Windows**: Azure Trusted Signing $9.99/mo Basic, open to individuals since Apr 2026 (EV certs $400-900/yr not needed).
- Homebrew: own tap first (main cask repo wants notability ~50-75+ stars), winget manifest PR (merged in days).

## MVP data-source scoping (v1, zero elevation)

1. Claude Code transcript tailing (~/.claude/projects/**/*.jsonl)
2. Claude Code hooks -> localhost HTTP (real-time, session attribution)
3. Cursor SQLite readers (state.vscdb + ~/.cursor/chats)
4. Lockfile/manifest watchers + diffing (npm/pnpm/pip/uv/cargo)
5. Process-poll attribution (1-2s interval, PPID walk)

v2 deep mode (opt-in elevation): ETW helper service on Windows; eslogger privileged helper on macOS; ES entitlement application in parallel. v3: network layer (NEFilterDataProvider / WFP).
