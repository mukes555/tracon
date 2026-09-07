/// Flag strings exactly as the Rust side emits them, so the UI tests break
/// if either side drifts. Sources: crates/tracon-adapters/src/danger.rs and
/// crates/tracon-ingest/src/intel.rs.
export const RUST_FLAGS = {
  destructiveDelete: "destructive delete",
  pipedScript: "remote script piped to shell",
  credentialAccess: "credential file access",
  forcePush: "force push",
  permissionsBypassed: "agent spawned with permissions bypassed",
  worldWritable: "world-writable permissions",
  rawDisk: "raw disk operation",
} as const;

/** Intel flag for a package with published advisories: "{name}: N known vulnerabilities (OSV)". */
export function vulnerabilityFlag(name: string, count: number): string {
  return `${name}: ${count} known vulnerabilities (OSV)`;
}

/** Intel flag for a suspiciously fresh package: "{name}: version published under 48h before install". */
export function freshPublishFlag(name: string): string {
  return `${name}: version published under 48h before install`;
}
