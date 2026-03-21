# Gale npm packages

This directory contains the npm distribution for Gale.

## Structure

```
npm/
  gale/                     Main wrapper package (gale-lint on npm)
    bin/gale                Node.js launcher script
    bin/postinstall.js      Verifies binary after install
    package.json
    README.md               Published to npm

  platforms/                Platform-specific binary packages (@aspect/gale-*)
    darwin-arm64/           macOS Apple Silicon
    darwin-x64/             macOS Intel
    linux-x64/              Linux x64
    linux-arm64/            Linux ARM64
    win32-x64/              Windows x64

  scripts/
    build-npm.sh            Local build helper
```

## How it works

Users install `gale-lint`, which declares platform packages as `optionalDependencies`.
npm/bun automatically installs only the package matching the user's OS and architecture.
The `bin/gale` wrapper script resolves and spawns the correct native binary.

## Publishing

Publishing is handled by the GitHub Actions release workflow (`.github/workflows/release.yml`).
Push a tag like `v0.1.0` to trigger the build and publish process.

To publish manually (not recommended):

```bash
cd npm/platforms/darwin-arm64 && npm publish --access public && cd ../../..
# ... repeat for each platform ...
cd npm/gale && npm publish --access public
```
