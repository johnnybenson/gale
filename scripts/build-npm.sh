#!/usr/bin/env bash
#
# Build and package Gale binaries for npm distribution.
#
# Usage:
#   ./scripts/build-npm.sh                 # Build for current platform only
#   ./scripts/build-npm.sh --all           # Build for all supported platforms (requires cross)
#   ./scripts/build-npm.sh --version 0.2.0 # Set version across all packages before building
#
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
NPM_DIR="$ROOT/npm"

# --------------------------------------------------------------------------
# Parse arguments
# --------------------------------------------------------------------------
BUILD_ALL=false
VERSION=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --all)
      BUILD_ALL=true
      shift
      ;;
    --version)
      VERSION="$2"
      shift 2
      ;;
    *)
      echo "Unknown argument: $1"
      echo "Usage: $0 [--all] [--version <semver>]"
      exit 1
      ;;
  esac
done

# --------------------------------------------------------------------------
# Version sync
# --------------------------------------------------------------------------
if [[ -n "$VERSION" ]]; then
  echo "==> Syncing version to $VERSION across all npm packages..."

  # Update main package version and its optionalDependencies
  cd "$NPM_DIR/gale-linter"
  npm version "$VERSION" --no-git-tag-version --allow-same-version 2>/dev/null

  # Update optionalDependencies to match
  node -e "
    const fs = require('fs');
    const pkg = JSON.parse(fs.readFileSync('package.json', 'utf8'));
    for (const dep of Object.keys(pkg.optionalDependencies || {})) {
      pkg.optionalDependencies[dep] = '$VERSION';
    }
    fs.writeFileSync('package.json', JSON.stringify(pkg, null, 2) + '\n');
  "

  # Update each platform package
  for dir in "$NPM_DIR"/@gale-linter/cli-*/; do
    if [[ -f "$dir/package.json" ]]; then
      cd "$dir"
      npm version "$VERSION" --no-git-tag-version --allow-same-version 2>/dev/null
    fi
  done

  echo "    All packages set to $VERSION"
fi

# --------------------------------------------------------------------------
# Target definitions: rust_target -> npm_platform_dir : binary_name
# --------------------------------------------------------------------------
declare -A TARGETS=(
  ["aarch64-apple-darwin"]="@gale-linter/cli-darwin-arm64:gale"
  ["x86_64-apple-darwin"]="@gale-linter/cli-darwin-x64:gale"
  ["x86_64-unknown-linux-gnu"]="@gale-linter/cli-linux-x64:gale"
  ["aarch64-unknown-linux-gnu"]="@gale-linter/cli-linux-arm64:gale"
  ["x86_64-unknown-linux-musl"]="@gale-linter/cli-linux-x64-musl:gale"
  ["aarch64-unknown-linux-musl"]="@gale-linter/cli-linux-arm64-musl:gale"
  ["x86_64-pc-windows-msvc"]="@gale-linter/cli-win32-x64:gale.exe"
)

# --------------------------------------------------------------------------
# Detect current platform's Rust target
# --------------------------------------------------------------------------
detect_current_target() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os-$arch" in
    Darwin-arm64)   echo "aarch64-apple-darwin" ;;
    Darwin-x86_64)  echo "x86_64-apple-darwin" ;;
    Linux-x86_64)   echo "x86_64-unknown-linux-gnu" ;;
    Linux-aarch64)  echo "aarch64-unknown-linux-gnu" ;;
    MINGW*-x86_64|MSYS*-x86_64|CYGWIN*-x86_64) echo "x86_64-pc-windows-msvc" ;;
    *)
      echo "ERROR: Cannot detect Rust target for $os-$arch" >&2
      exit 1
      ;;
  esac
}

# --------------------------------------------------------------------------
# Build a single target
# --------------------------------------------------------------------------
build_target() {
  local rust_target="$1"
  local npm_info="${TARGETS[$rust_target]}"
  local npm_dir="${npm_info%%:*}"
  local binary_name="${npm_info##*:}"
  local use_cross="$2"

  echo "==> Building for $rust_target..."

  if [[ "$use_cross" == "true" ]]; then
    cross build --release --target "$rust_target" --manifest-path "$ROOT/Cargo.toml"
    local src="$ROOT/target/$rust_target/release/$binary_name"
  else
    cargo build --release --manifest-path "$ROOT/Cargo.toml"
    local src="$ROOT/target/release/$binary_name"
  fi

  local dest="$NPM_DIR/$npm_dir/$binary_name"
  echo "    Copying $src -> $dest"
  cp "$src" "$dest"
  chmod +x "$dest"
  echo "    Done: $rust_target -> $npm_dir/$binary_name"
}

# --------------------------------------------------------------------------
# Main
# --------------------------------------------------------------------------
if [[ "$BUILD_ALL" == "true" ]]; then
  echo "==> Building for ALL platforms (requires 'cross' — install with: cargo install cross)"
  echo ""

  # Check for cross
  if ! command -v cross &>/dev/null; then
    echo "ERROR: 'cross' is not installed."
    echo "Install it with: cargo install cross"
    echo ""
    echo "You also need Docker running for cross-compilation."
    exit 1
  fi

  CURRENT_TARGET="$(detect_current_target)"

  for rust_target in "${!TARGETS[@]}"; do
    if [[ "$rust_target" == "$CURRENT_TARGET" ]]; then
      # Native build — no need for cross
      build_target "$rust_target" "false"
    else
      build_target "$rust_target" "true"
    fi
    echo ""
  done
else
  CURRENT_TARGET="$(detect_current_target)"
  build_target "$CURRENT_TARGET" "false"
fi

echo ""
echo "=========================================="
echo " Build complete!"
echo "=========================================="
echo ""
echo "Packages ready in: $NPM_DIR/"
echo ""
echo "To publish (platform packages first, then main package):"
echo ""
echo "  # 1. Publish platform packages"
echo "  for dir in $NPM_DIR/@gale-linter/cli-*/; do"
echo "    (cd \"\$dir\" && npm publish --access public)"
echo "  done"
echo ""
echo "  # 2. Publish main package"
echo "  (cd $NPM_DIR/gale-linter && npm publish --access public)"
echo ""
