# Homebrew Formula (save as remote-tool.rb in tap repo)

class RemoteTool < Formula
  desc "High-performance Rust CLI for remote SSH operations"
  homepage "https://github.com/yourusername/remote-tool"
  url "https://github.com/yourusername/remote-tool/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "compute_sha_here"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    system "#{bin}/remote-tool", "--version"
  end
end