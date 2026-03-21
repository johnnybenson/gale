# Publishing Gale to npm

Gale is distributed on npm as precompiled platform-specific binaries, following the
same pattern used by [Biome](https://biomejs.dev) and other Rust-to-npm tools.

## Package layout

| npm package | Contents |
|---|---|
| `gale-linter` | Main package with JS wrapper + postinstall. Declares platform packages as `optionalDependencies`. |
| `@gale-linter/cli-darwin-arm64` | macOS Apple Silicon binary |
| `@gale-linter/cli-darwin-x64` | macOS Intel binary |
| `@gale-linter/cli-linux-x64` | Linux x64 (glibc) binary |
| `@gale-linter/cli-linux-arm64` | Linux ARM64 (glibc) binary |
| `@gale-linter/cli-linux-x64-musl` | Linux x64 (musl/Alpine) binary |
| `@gale-linter/cli-linux-arm64-musl` | Linux ARM64 (musl/Alpine) binary |
| `@gale-linter/cli-win32-x64` | Windows x64 binary |

When a user runs `bun install gale-linter`, their package manager resolves only the
`optionalDependency` matching their OS/arch. The `bin/gale` wrapper script uses
`require.resolve()` to find the platform binary at runtime.

## Prerequisites

- npm account with publish access to the `@gale-linter` scope
- Create the scope on npmjs.com first: https://www.npmjs.com/org/create
- For cross-compilation: `cargo install cross` + Docker

## First-time setup

```bash
# 1. Create the @gale-linter org on npm (do this once)
npm org create gale-linter

# 2. Log in to npm
npm login

# 3. Verify you can publish to the scope
npm access ls-packages @gale-linter
```

## Release process

### Option A: Automated (GitHub Actions)

Push a git tag matching `v*` (e.g. `v0.2.0`) to trigger the release workflow:

```bash
git tag v0.2.0
git push --tags
```

The workflow will:
1. Build binaries for all platforms in parallel
2. Update version numbers
3. Publish platform packages
4. Publish the main `gale-linter` package
5. Create a GitHub Release with the binaries attached

### Option B: Manual

```bash
# 1. Set the version across all packages
./scripts/build-npm.sh --version 0.2.0

# 2. Build for current platform (or --all with cross + Docker)
./scripts/build-npm.sh

# 3. Publish platform packages first — they must exist before the main package
for dir in npm/@gale-linter/cli-*/; do
  (cd "$dir" && npm publish --access public)
done

# 4. Publish the main package
(cd npm/gale-linter && npm publish --access public)
```

### Option C: CI matrix build (recommended for production)

For best results, build each platform natively in CI rather than cross-compiling.
Each CI job builds one target, then a final job publishes everything.

Example matrix:

| Runner | Rust target | npm package |
|---|---|---|
| `macos-14` (ARM) | `aarch64-apple-darwin` | `@gale-linter/cli-darwin-arm64` |
| `macos-13` (Intel) | `x86_64-apple-darwin` | `@gale-linter/cli-darwin-x64` |
| `ubuntu-latest` | `x86_64-unknown-linux-gnu` | `@gale-linter/cli-linux-x64` |
| `ubuntu-latest` + cross | `aarch64-unknown-linux-gnu` | `@gale-linter/cli-linux-arm64` |
| `ubuntu-latest` + cross | `x86_64-unknown-linux-musl` | `@gale-linter/cli-linux-x64-musl` |
| `ubuntu-latest` + cross | `aarch64-unknown-linux-musl` | `@gale-linter/cli-linux-arm64-musl` |
| `windows-latest` | `x86_64-pc-windows-msvc` | `@gale-linter/cli-win32-x64` |

## Rust targets reference

| npm platform | Rust target | Build tool |
|---|---|---|
| darwin-arm64 | `aarch64-apple-darwin` | `cargo` (native on M1+) |
| darwin-x64 | `x86_64-apple-darwin` | `cargo` (native or `rustup target add`) |
| linux-x64 | `x86_64-unknown-linux-gnu` | `cargo` (native) or `cross` |
| linux-arm64 | `aarch64-unknown-linux-gnu` | `cross` |
| linux-x64-musl | `x86_64-unknown-linux-musl` | `cross` |
| linux-arm64-musl | `aarch64-unknown-linux-musl` | `cross` |
| win32-x64 | `x86_64-pc-windows-msvc` | `cargo` (native on Windows) or `cross` |

## Version management

All packages must have the same version. The build script handles this:

```bash
./scripts/build-npm.sh --version 0.2.0
```

This updates:
- `npm/gale-linter/package.json` — version + all `optionalDependencies` versions
- `npm/@gale-linter/cli-*/package.json` — version in each platform package

## Testing locally

```bash
# 1. Build for your current platform
./scripts/build-npm.sh

# 2. Pack (dry run) to see what would be published
(cd npm/gale-linter && npm pack --dry-run)
(cd npm/@gale-linter/cli-darwin-arm64 && npm pack --dry-run)

# 3. Test the full install flow locally
mkdir /tmp/gale-test && cd /tmp/gale-test
npm init -y
npm install /path/to/gale/npm/gale-linter
npx gale --version
```

## Troubleshooting

**"Platform package was not installed"**: Some package managers skip optional
dependencies. Users can set `GALE_BINARY=/path/to/gale` as a workaround.

**musl detection**: The wrapper script detects musl by running `ldd --version`
and checking for "musl" in the output. Alpine Linux and similar distros will
get the musl variant automatically.

**Binary not executable**: The build script sets `chmod +x` on the binary.
If publishing from Windows, ensure the execute bit is preserved.
