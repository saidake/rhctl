#!/bin/bash
# Generate .deb package
# Requires cargo-deb

set -e

cargo deb
# Outputs target/debian/remote-tool_0.1.0_amd64.deb