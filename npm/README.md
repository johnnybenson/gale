# Gale npm packages

This directory contains the npm distribution for Gale, following the same
`optionalDependencies` pattern used by [Biome](https://biomejs.dev).

## Structure

```
npm/
  gale-linter/                  Main wrapper package (published as "gale-linter")
    bin/gale                    Node.js launcher script
    bin/postinstall.js          Verifies binary after install
    package.json
    README.md                   Published to npm

  @gale-linter/                 Platform-specific binary packages
    cli-darwin-arm64/           macOS Apple Silicon
    cli-darwin-x64/             macOS Intel
    cli-linux-x64/              Linux x64 (glibc)
    cli-linux-arm64/            Linux ARM64 (glibc)
    cli-linux-x64-musl/         Linux x64 (musl / Alpine)
    cli-linux-arm64-musl/       Linux ARM64 (musl / Alpine)
    cli-win32-x64/              Windows x64
```

## How it works

1. Users install `gale-linter`, which declares platform packages as `optionalDependencies`
2. npm/bun/yarn automatically installs only the package matching the user's OS and architecture
3. The `bin/gale` wrapper script resolves and spawns the correct native binary
4. The `postinstall` script verifies the binary works and prints the installed version

## Building

```bash
# Build for current platform
./scripts/build-npm.sh

# Build for all platforms (requires `cross` and Docker)
./scripts/build-npm.sh --all

# Set version and build
./scripts/build-npm.sh --version 0.2.0
```

## Publishing

Publishing is handled by the GitHub Actions release workflow.
Push a tag like `v0.1.0` to trigger the build and publish process.

To publish manually:

```bash
# 1. Set version
./scripts/build-npm.sh --version 0.2.0

# 2. Build binaries (--all for cross-platform, or run on each CI platform)
./scripts/build-npm.sh --all

# 3. Publish platform packages first (order matters!)
for dir in npm/@gale-linter/cli-*/; do
  (cd "$dir" && npm publish --access public)
done

# 4. Publish main package
(cd npm/gale-linter && npm publish --access public)
```

## Legacy structure

The `gale/` and `platforms/` directories contain the previous npm setup using the
`@aspect/` scope. They are kept for reference but the new `gale-linter` and
`@gale-linter/` packages should be used going forward.
