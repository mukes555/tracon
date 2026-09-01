# Packaging

Distribution scaffolds, activated at first public release.

- `homebrew/tracon.rb`: cask formula for our own tap (`brew install tracon-dev/tap/tracon`). Fill in the version, url, and sha256 from the GitHub release; graduate to homebrew-cask main once the project meets their notability bar.
- `winget/`: manifest for a PR to microsoft/winget-pkgs. Needs a stable installer URL from a published GitHub release and (strongly preferred) a signed installer via Azure Trusted Signing.

Release flow: tag `vX.Y.Z` -> CI builds draft release with installers -> publish release -> update these manifests with the final URLs/hashes.
