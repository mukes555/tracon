# Contributing to Tracon

Thanks for helping build the flight recorder for AI coding agents.

## Ground rules

- The desktop recorder is and will remain free and AGPL-3.0. Paid features will only ever be team/server-side (a reserved `ee/` area). Contributions to the open code are licensed AGPL-3.0.
- By submitting a contribution you certify the Developer Certificate of Origin (DCO): sign off your commits with `git commit -s`.
- Recording fidelity and safety signals are never gated, degraded, or monetized. PRs that do so will be declined.

## Development setup

Prereqs: Rust stable, Node 20+, pnpm 10 (via corepack).

```
pnpm install
cargo test --workspace
pnpm --dir apps/desktop tauri dev
```

## Code style

- Rust: `cargo fmt` and `cargo clippy --workspace` must pass.
- Write for human readers: small files split by responsibility, early returns, intermediate variables with meaningful names instead of dense conditionals, deep modules over shallow ones.
- Comments explain WHY, not WHAT.
- Conventional Commits for messages (`feat:`, `fix:`, `docs:`, ...).

## The one invariant

Tracon observes; it never interferes. Any code path that could block, slow, or alter an agent's behavior (a synchronous hook decision, a non-2xx response, a long timeout) is a bug unless it is an explicit, user-enabled feature.
