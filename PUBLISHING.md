# Publishing Gale to npm

Gale is distributed on npm as `@lyricalstring/gale`. The package uses a postinstall
script that downloads the correct precompiled binary from GitHub Releases.

## Package layout

```
npm/
  package.json    @lyricalstring/gale — main package
  install.js      Postinstall script (downloads platform binary from GitHub Releases)
  bin/gale        Placeholder script (replaced by real binary on install)
  README.md       npm page README
```

When a user runs `npm install @lyricalstring/gale`, the postinstall script
(`install.js`) detects the platform and downloads the matching binary from the
GitHub Release matching the package version.

Supported platforms: `darwin-arm64`, `darwin-x64`, `linux-arm64`, `linux-x64`.

## Prerequisites

- npm account with publish access to the `@lyricalstring` scope
- GitHub Release with precompiled binaries for the target version

## Release process

### Option A: Automated (GitHub Actions)

Push a git tag matching `v*` (e.g. `v0.2.0`) to trigger the release workflow:

```bash
# 1. Update workspace.package.version in Cargo.toml
# 2. Commit the version bump
git tag v0.2.0
git push && git push --tags
```

The workflow will:
1. Build Linux binaries (x64 + arm64)
2. Create a GitHub Release with the binaries
3. Set the npm package version to match
4. Publish `@lyricalstring/gale` to npm

### Option B: Manual

```bash
# 1. Build the binary for your platform
./scripts/build-npm.sh

# 2. Set the version
./scripts/build-npm.sh --version 0.2.0

# 3. Publish
cd npm && npm publish --access public
```

Note: For users to install successfully, the GitHub Release for the matching
version must contain binaries for all supported platforms. Use the CI release
workflow for production releases.

## Rust targets reference

| Platform | Rust target | GitHub Release artifact name |
|---|---|---|
| macOS ARM (M1+) | `aarch64-apple-darwin` | `gale-aarch64-apple-darwin` |
| macOS Intel | `x86_64-apple-darwin` | `gale-x86_64-apple-darwin` |
| Linux x64 | `x86_64-unknown-linux-gnu` | `gale-x86_64-unknown-linux-gnu` |
| Linux ARM64 | `aarch64-unknown-linux-gnu` | `gale-aarch64-unknown-linux-gnu` |

## Crates.io

The Rust crate is published separately as `gale-lint` on crates.io (since the
name `gale` was already taken). The binary it installs is still called `gale`.

```bash
cargo install gale-lint
```

## Testing locally

```bash
# 1. Build for your current platform
./scripts/build-npm.sh

# 2. Pack (dry run) to see what would be published
cd npm && npm pack --dry-run

# 3. Test the full install flow locally
mkdir /tmp/gale-test && cd /tmp/gale-test
npm init -y
npm install /path/to/gale/npm
npx gale --version
```

## Troubleshooting

**"Gale binary not found"**: The postinstall download may have failed. Run
`npm rebuild @lyricalstring/gale` to retry, or check network access to
`github.com`.

**Unsupported platform**: The postinstall script only supports darwin and linux
on arm64/x64. Windows is not yet supported via npm. Build from source instead.
