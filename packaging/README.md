# Packaging

Distribution scaffolds, activated at first public release.

- `homebrew/tracon.rb.tmpl`: the cask template for our tap (`brew install --cask mukes555/tap/tracon`). The tap workflow (`.github/workflows/tap.yml`) renders it with the release checksums when a release is published; graduate to homebrew-cask main once the project meets their notability bar.
- `winget/`: manifest for a PR to microsoft/winget-pkgs. Needs a stable installer URL from a published GitHub release and (strongly preferred) a signed installer via Azure Trusted Signing.

Release flow: tag `vX.Y.Z` -> CI builds draft release with installers -> publish release -> update these manifests with the final URLs/hashes.
