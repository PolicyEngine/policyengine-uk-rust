#!/usr/bin/env bash
# Build the Rust binary and stage it + parameters into the Python package.
# Usage: ./build_package.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PKG_DIR="$SCRIPT_DIR/policyengine_uk_compiled"

echo "Building Rust binary (release)..."
cargo build --release

echo "Staging binary into package..."
mkdir -p "$PKG_DIR/bin"
cp "$SCRIPT_DIR/target/release/policyengine-uk-rust" "$PKG_DIR/bin/"
chmod +x "$PKG_DIR/bin/policyengine-uk-rust"

echo "Staging parameters into package..."
rm -rf "$PKG_DIR/parameters"
cp -r "$SCRIPT_DIR/parameters" "$PKG_DIR/parameters"

echo "Done. Build the wheel with: python -m build"
