#!/bin/bash
# Automated script for GitHub releases

set -e

VERSION=$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[0].version')
TARGETS=("x86_64-unknown-linux-gnu" "x86_64-apple-darwin" "aarch64-apple-darwin" "x86_64-pc-windows-msvc")

for TARGET in "${TARGETS[@]}"; do
  cross build --release --target $TARGET
  zip -j "rsctl-${VERSION}-${TARGET}.zip" "target/${TARGET}/release/rsctl"
done

# Use gh CLI to create release
gh release create "v${VERSION}" --title "Release v${VERSION}" --notes "Automated release" *.zip