# Homebrew formula for grite
# This file is auto-updated by the release workflow

class Grite < Formula
  desc "Git-backed issue tracking for coding agents and humans"
  homepage "https://github.com/neul-labs/grite"
  version "0.3.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/neul-labs/grite/releases/download/v#{version}/grite-#{version}-aarch64-apple-darwin.tar.gz"
      # sha256 will be updated by release workflow
      sha256 "PLACEHOLDER_ARM64_SHA256"
    else
      url "https://github.com/neul-labs/grite/releases/download/v#{version}/grite-#{version}-x86_64-apple-darwin.tar.gz"
      # sha256 will be updated by release workflow
      sha256 "PLACEHOLDER_X64_SHA256"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/neul-labs/grite/releases/download/v#{version}/grite-#{version}-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "PLACEHOLDER_LINUX_ARM64_SHA256"
    else
      url "https://github.com/neul-labs/grite/releases/download/v#{version}/grite-#{version}-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "PLACEHOLDER_LINUX_X64_SHA256"
    end
  end

  depends_on "nng"

  def install
    bin.install "grite"
    bin.install "grite-daemon"
  end

  test do
    system "#{bin}/grite", "--version"
  end
end
