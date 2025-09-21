#!/bin/bash
# Automated script for GitHub releases
# Assumes cargo cross for cross-compilation

set -e

VERSION=$(cargo metadata --no-deps --format-version 1)
TARGETS=("x86_64-unknown-linux-gnu" "x86_64-apple-darwin" "aarch64-apple-darwin" "x86_64-pc-windows-msvc")

for TARGET in "${TARGETS[@]}"; do
  cross build --release --target $TARGET
  zip -j "remote-tool-${VERSION}-${TARGET}.zip" "target/${TARGET}/release/remote-tool"
done

# Use gh CLI to create release
gh release create "v${VERSION}" --title "Release v${VERSION}" --notes "Automated release" *.zip