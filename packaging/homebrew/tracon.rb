# Cask for the tracon-dev/tap Homebrew tap. Placeholders are filled by the
# release process; do not publish with TODO values.
cask "tracon" do
  version "0.1.0" # TODO: sync with release tag
  sha256 "TODO_SHA256_OF_DMG"

  url "https://github.com/tracon-dev/tracon/releases/download/v#{version}/Tracon_#{version}_aarch64.dmg"
  name "Tracon"
  desc "Flight recorder for AI coding agents"
  homepage "https://github.com/tracon-dev/tracon"

  app "Tracon.app"

  zap trash: [
    "~/Library/Application Support/dev.tracon.desktop",
    "~/.tracon",
  ]
end
