#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

# Detect current platform
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Darwin)
    PLATFORM_OS="darwin"
    ;;
  Linux)
    PLATFORM_OS="linux"
    ;;
  MINGW*|MSYS*|CYGWIN*)
    PLATFORM_OS="win32"
    ;;
  *)
    echo "Unsupported OS: $OS"
    exit 1
    ;;
esac

case "$ARCH" in
  x86_64|amd64)
    PLATFORM_ARCH="x64"
    ;;
  aarch64|arm64)
    PLATFORM_ARCH="arm64"
    ;;
  *)
    echo "Unsupported architecture: $ARCH"
    exit 1
    ;;
esac

PLATFORM_DIR="cli-${PLATFORM_OS}-${PLATFORM_ARCH}"
BINARY_NAME="gale"
if [ "$PLATFORM_OS" = "win32" ]; then
  BINARY_NAME="gale.exe"
fi

echo "Building Gale for ${PLATFORM_OS}-${PLATFORM_ARCH}..."
cargo build --release --manifest-path "$ROOT/Cargo.toml"

echo "Copying binary to npm/${PLATFORM_DIR}/${BINARY_NAME}..."
cp "$ROOT/target/release/${BINARY_NAME}" "$ROOT/npm/${PLATFORM_DIR}/${BINARY_NAME}"

echo "Done! Binary placed in npm/${PLATFORM_DIR}/${BINARY_NAME}"
echo ""
echo "To build for other platforms, use cross-compilation:"
echo ""
echo "  # Install cross (if not installed)"
echo "  cargo install cross"
echo ""
echo "  # Build for Linux x64"
echo "  cross build --release --target x86_64-unknown-linux-gnu"
echo "  cp target/x86_64-unknown-linux-gnu/release/gale npm/cli-linux-x64/gale"
echo ""
echo "  # Build for Linux ARM64"
echo "  cross build --release --target aarch64-unknown-linux-gnu"
echo "  cp target/aarch64-unknown-linux-gnu/release/gale npm/cli-linux-arm64/gale"
echo ""
echo "  # Build for macOS x64 (from macOS ARM64)"
echo "  rustup target add x86_64-apple-darwin"
echo "  cargo build --release --target x86_64-apple-darwin"
echo "  cp target/x86_64-apple-darwin/release/gale npm/cli-darwin-x64/gale"
echo ""
echo "  # Build for Windows x64"
echo "  cross build --release --target x86_64-pc-windows-msvc"
echo "  cp target/x86_64-pc-windows-msvc/release/gale.exe npm/cli-win32-x64/gale.exe"
