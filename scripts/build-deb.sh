#!/bin/bash
# Generate .deb package
# Requires cargo-deb
set -e
cargo deb
# Outputs target/debian/sbxctl_0.1.0-1_amd64.deb


# sudo dpkg -i ../target/debian/sbxctl_0.1.0-1_amd64.deb