# Homebrew formula for grit
# This file is auto-updated by the release workflow

class Grit < Formula
  desc "Git-backed issue tracking for coding agents and humans"
  homepage "https://github.com/neul-labs/grit"
  version "0.1.0"
  license any_of: ["MIT", "Apache-2.0"]

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/neul-labs/grit/releases/download/v#{version}/grit-#{version}-aarch64-apple-darwin.tar.gz"
      # sha256 will be updated by release workflow
      sha256 "PLACEHOLDER_ARM64_SHA256"
    else
      url "https://github.com/neul-labs/grit/releases/download/v#{version}/grit-#{version}-x86_64-apple-darwin.tar.gz"
      # sha256 will be updated by release workflow
      sha256 "PLACEHOLDER_X64_SHA256"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/neul-labs/grit/releases/download/v#{version}/grit-#{version}-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "PLACEHOLDER_LINUX_ARM64_SHA256"
    else
      url "https://github.com/neul-labs/grit/releases/download/v#{version}/grit-#{version}-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "PLACEHOLDER_LINUX_X64_SHA256"
    end
  end

  depends_on "nng"

  def install
    bin.install "grit"
    bin.install "grited"
  end

  test do
    system "#{bin}/grit", "--version"
  end
end
