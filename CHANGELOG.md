# Changelog

All notable changes to Tracon. The format follows Keep a Changelog; versions follow SemVer.

## Unreleased

### Security
- The ingest server now requires a per-install token on every POST and checks the Host header, so other local processes and rebound web pages cannot forge or suppress events.
- Session ids from hooks and transcripts are sanitized before they touch any file path.
- The Claude hook spool is capped at 50 MB, created with mode 0600, and carries real timestamps.
- Threat intel validates package names before any network use and never retries permanent failures forever.
- Every release installer carries a Sigstore build provenance attestation.

### Fixed
- Today's counts use local time, matching the timeline.
- A failed poll no longer freezes the UI; a banner shows when the recorder cannot be reached.
- Acknowledging flags reports failure honestly instead of showing success.
- A malformed filename can no longer stop Codex capture until restart.
- Retention purges in batches and reclaims disk space.

### Added
- Capture paused is visible in the UI, with a Resume button and a Settings switch.
- Delete everything and delete one session, from Settings and the session inspector.
- App version and a log file, revealed from Settings.
- Frontend tests (Vitest), Linux in CI, dependency audits, a pinned toolchain.

## 0.3.0 (2026-09-07)

### Added
- The quokka: app icon, menu bar template icon, nav avatar, and illustrated empty states.
- Native shell with a command bar, a Simple and Advanced switch, and status strips.
- One row anatomy across Timeline, Flagged, Packages, and Overview.
- Docked inspector on wide windows with session context, flag explanations, and prev and next.
- Keyboard flow: arrows or j and k, Enter, Esc, A to acknowledge and advance, 1 to 6 for pages, undo toasts.
- Flagged triage grouped by severity with acknowledge-all per group.
- Homebrew cask in the mukes555/tap tap.

### Fixed
- Keyboard stepping walks the rows on screen, including the acknowledged bucket.
- Acknowledge-all covers the whole group in one transaction.
- Flag families match the reason part of intel flags.

## 0.2.0 (2026-09-02)

### Added
- Live page: one monitor per active session, streaming recent commands.
- Session-level live board on the Overview with subagent visibility.

### Fixed
- Subagent counts, live flag chips, live card titles, and the live query cost.

## 0.1.0 (2026-09-01)

First public release: Claude Code, Codex, Cursor, and Gemini capture, danger flags, package watch with opt-in threat intel, conversation reader, timeline, and search.
