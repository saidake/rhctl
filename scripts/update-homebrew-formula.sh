#!/usr/bin/env bash
# Generate Formula/rhctl.rb from release binaries.
#
# Usage:
#   ./scripts/update-homebrew-formula.sh <version> <dist_dir> [output_path]
#
# Example:
#   ./scripts/update-homebrew-formula.sh 1.0.1 dist Formula/rhctl.rb
set -euo pipefail

if [[ $# -lt 2 || $# -gt 3 ]]; then
  echo "Usage: $0 <version> <dist_dir> [output_path]" >&2
  exit 1
fi

VERSION="$1"
DIST_DIR="$2"
OUTPUT="${3:-Formula/rhctl.rb}"

sha256_file() {
  local file="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$file" | awk '{print $1}'
  else
    shasum -a 256 "$file" | awk '{print $1}'
  fi
}

LINUX="${DIST_DIR}/rhctl-v${VERSION}-linux-x86_64"
MACOS_X64="${DIST_DIR}/rhctl-v${VERSION}-macos-x86_64"
MACOS_ARM64="${DIST_DIR}/rhctl-v${VERSION}-macos-arm64"

for f in "$LINUX" "$MACOS_X64" "$MACOS_ARM64"; do
  if [[ ! -f "$f" ]]; then
    echo "Missing binary: $f" >&2
    exit 1
  fi
done

LINUX_SHA="$(sha256_file "$LINUX")"
MACOS_X64_SHA="$(sha256_file "$MACOS_X64")"
MACOS_ARM64_SHA="$(sha256_file "$MACOS_ARM64")"

BASE_URL="https://github.com/saidake/rhctl/releases/download/v${VERSION}"

mkdir -p "$(dirname "$OUTPUT")"

cat >"$OUTPUT" <<EOF
class Rhctl < Formula
  desc "High-performance Rust CLI tool for remote host management"
  homepage "https://github.com/saidake/rhctl"
  version "${VERSION}"
  license "GPL-3.0-or-later"

  on_macos do
    on_arm do
      url "${BASE_URL}/rhctl-v${VERSION}-macos-arm64"
      sha256 "${MACOS_ARM64_SHA}"
    end
    on_intel do
      url "${BASE_URL}/rhctl-v${VERSION}-macos-x86_64"
      sha256 "${MACOS_X64_SHA}"
    end
  end

  on_linux do
    on_intel do
      url "${BASE_URL}/rhctl-v${VERSION}-linux-x86_64"
      sha256 "${LINUX_SHA}"
    end
  end

  def install
    bin.install Dir["rhctl*"].first => "rhctl"
  end

  test do
    assert_match "Usage", shell_output("#{bin}/rhctl --help")
  end
end
EOF

echo "Wrote ${OUTPUT}"
