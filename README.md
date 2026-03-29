# Gale

**An extremely fast CSS linter. Drop-in replacement for Stylelint.**

[![npm version](https://img.shields.io/npm/v/@lyricalstring/gale)](https://www.npmjs.com/package/@lyricalstring/gale)
[![CI](https://github.com/LyricalString/gale/actions/workflows/ci.yml/badge.svg)](https://github.com/LyricalString/gale/actions)
[![license: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Gale reads your existing `.stylelintrc`, runs the same rules, and produces the same output. Just **100x-400x faster**.

One line change in your `package.json`. No config migration.

## Benchmarks

Real-world benchmarks using [hyperfine](https://github.com/sharkdp/hyperfine) (10 runs, 3 warmup). Each repo uses its own Stylelint config.

| Repository | Files | Stylelint | Gale | Speedup |
|------------|------:|----------:|-----:|--------:|
| [Bootstrap](https://github.com/twbs/bootstrap) | 99 | 1.569s | 0.011s | **143x** |
| [Carbon](https://github.com/carbon-design-system/carbon) | 1,116 | 8.767s | 0.022s | **399x** |
| [PatternFly](https://github.com/patternfly/patternfly) | 204 | 5.298s | 0.015s | **353x** |
| [Spectrum CSS](https://github.com/adobe/spectrum-css) | 241 | 2.952s | 0.011s | **268x** |
| [GOV.UK Frontend](https://github.com/alphagov/govuk-frontend) | 163 | 1.614s | 0.011s | **147x** |
| [Primer CSS](https://github.com/primer/css) | 113 | 1.285s | 0.010s | **129x** |
| [Gutenberg](https://github.com/wordpress/gutenberg) | 775 | 4.715s | 0.042s | **112x** |
| [wp-calypso](https://github.com/Automattic/wp-calypso) | 2,238 | 13.223s | 0.131s | **101x** |
| [Angular Components](https://github.com/angular/components) | 620 | 1.843s | 0.021s | **88x** |
| [Discourse](https://github.com/discourse/discourse) | 355 | 1.438s | 0.021s | **69x** |

**Parity: 0 false positives and 0 false negatives across all 22 tested repositories (5,790 files).**

Reproduce these results: `./benchmarks/benchmark.sh`

## Quick start

```bash
# Install
npm install -D @lyricalstring/gale

# Lint (uses your existing .stylelintrc)
npx gale "src/**/*.css"

# Autofix
npx gale --fix "src/**/*.css"
```

## Migrate from Stylelint

Change one line in `package.json`:

```diff
 {
   "scripts": {
-    "lint:css": "stylelint 'src/**/*.css'"
+    "lint:css": "gale 'src/**/*.css'"
   }
 }
```

Your `.stylelintrc` stays exactly the same. Gale reads the same config files, follows the same `extends` chains, honors `/* stylelint-disable */` comments, and produces the same JSON output format.

## Installation

### npm (recommended)

```bash
npm install -D @lyricalstring/gale
```

The npm package automatically downloads the correct platform binary on install. Supported platforms: macOS (arm64, x64), Linux (x64, arm64).

### Cargo

```bash
cargo install gale-lint
```

The crate is named `gale-lint` on crates.io (since `gale` was taken), but the installed binary is called `gale`.

### From source

```bash
git clone https://github.com/LyricalString/gale.git
cd gale
cargo build --release
# Binary at target/release/gale
```

### GitHub releases

Download pre-built binaries from [GitHub Releases](https://github.com/LyricalString/gale/releases).

## What's supported

### 260+ built-in rules

Gale ships 260+ built-in rules across four categories:

| Category | Count | Examples |
|----------|------:|---------|
| Core Stylelint | 144 | `block-no-empty`, `color-no-invalid-hex`, `property-no-unknown` |
| SCSS (`scss/*`) | 44 | `scss/at-rule-no-unknown`, `scss/no-duplicate-mixins`, `scss/dollar-variable-pattern` |
| Stylistic (`@stylistic/*`) | 59 | `stylistic/indentation`, `stylistic/declaration-colon-space-after`, `stylistic/no-eol-whitespace` |
| Order (`order/*`) | 3 | `order/order`, `order/properties-order`, `order/properties-alphabetical-order` |

SCSS and stylistic rules are built in -- no extra plugins required.

### Config compatibility

All Stylelint config formats are supported:

| File | Format |
|------|--------|
| `gale.json` | JSON (native) |
| `gale.toml` | TOML (native) |
| `.stylelintrc` | JSON or YAML |
| `.stylelintrc.json` | JSON |
| `.stylelintrc.yml` / `.yaml` | YAML |
| `stylelint.config.js` / `.cjs` | JavaScript |

### Feature overview

- **SCSS and Less** out of the box (no plugins needed)
- **Autofix** via `--fix`
- **File caching** via `--cache` (skips unchanged files)
- **LSP server** for editor integration (`--lsp`)
- **Parallel linting** using all CPU cores
- **Inline disable comments** (`stylelint-disable` and `gale-disable`)
- **JSON, text, and compact** output formatters matching Stylelint's format
- **`extends`** with built-in presets, npm packages, and relative paths
- **`.galeignore`** files (gitignore syntax) for custom exclusions

### Not yet supported

- **Custom JavaScript plugins.** Third-party rule packages (community plugins) are not supported. Gale only runs its built-in rules.
- **`package.json` config.** The `"stylelint"` field in `package.json` is not read.
- **Sass indented syntax.** `.sass` files are not supported (`.scss` works fine).

## Configuration

Gale searches for config files walking up from the working directory. To generate a starter config:

```bash
npx gale --init
```

### Example config

```json
{
  "extends": "gale:recommended",
  "rules": {
    "block-no-empty": true,
    "color-hex-length": "warning",
    "number-max-precision": ["error", { "max": 4 }],
    "declaration-no-important": "off"
  }
}
```

### Rule value formats

| Format | Meaning |
|--------|---------|
| `true` | Enable at error severity |
| `false` or `"off"` | Disable |
| `"error"` | Enable at error severity |
| `"warning"` | Enable at warning severity |
| `["error", { options }]` | Enable with options |

### Built-in presets

| Preset | Description |
|--------|-------------|
| `gale:recommended` | Sensible defaults (29 rules: 15 error + 14 warning) |
| `gale:all` | All rules enabled at warning severity |

You can also extend npm packages like `stylelint-config-standard` directly.

### Extends resolution

The `extends` field supports:

| Value | Resolution |
|-------|------------|
| `"gale:recommended"` | Built-in preset |
| `"gale:all"` | Built-in preset |
| `"./path/to/config.json"` | Relative path to another config file |
| `"stylelint-config-standard"` | npm package (resolved from `node_modules/`) |

Resolution is recursive with cycle detection. Later `extends` entries override earlier ones. User `rules` always override extended rules.

## CLI reference

```
gale [OPTIONS] [FILES]...
```

| Flag | Description |
|------|-------------|
| `<files>` | Files, directories, or glob patterns to lint |
| `--fix` | Automatically fix problems |
| `-q, --quiet` | Only report errors |
| `-f, --formatter <type>` | Output: `text` (default), `json`, `compact` |
| `-c, --config <path>` | Config file path |
| `--max-warnings <n>` | Error if warnings exceed threshold |
| `--cache` | Skip unchanged files |
| `--cache-location <path>` | Custom cache file path (default: `.gale_cache`) |
| `--stdin` | Read from stdin |
| `--stdin-filename <name>` | Virtual filename for stdin (default: `stdin.css`) |
| `--ignore-path <file>` | Custom ignore file (gitignore syntax) |
| `--no-ignore` | Disable all ignore file processing |
| `--print-config <file>` | Print resolved config as JSON |
| `--init` | Generate starter config |
| `--lsp` | Start LSP server |

## Editor integration

### Any LSP-compatible editor

```bash
gale --lsp
```

Works with Neovim, Helix, Zed, and any editor supporting the Language Server Protocol.

## Development

### Prerequisites

- Rust 2024 edition (1.85+)
- Python 3 (for differential tests)
- Node.js 16+ (for differential tests and npm packaging)

### Build and test

```bash
cargo build                          # Debug build
cargo build --release                # Release build
cargo test --workspace               # Run all tests
cargo test -p gale_linter            # Tests for a specific crate
cargo test -p gale_linter block_no_empty  # A specific test
cargo clippy --workspace -- -D warnings   # Lint the Rust code
cargo fmt --check                    # Check formatting
```

### Run the linter

```bash
cargo run -- "src/**/*.css"          # Lint
cargo run -- --fix "src/**/*.css"    # Autofix
cargo run -- --formatter json src/   # JSON output
```

### Debug and profiling

```bash
GALE_DEBUG_PERF=1 cargo run --release -- src/   # Per-phase timings to stderr
GALE_LOG=debug cargo run -- src/                # Tracing/logging output
```

### Differential testing

Compare Gale output against Stylelint on real-world repositories:

```bash
python tests/differential/run.py              # All repos
python tests/differential/run.py bootstrap    # Specific repo
python tests/differential/run.py --benchmark  # Include timing comparison
python tests/differential/run.py --list       # List available repos
python tests/differential/run.py --css-only   # Skip SCSS/Less
python tests/differential/run.py --skip-build # Use existing binary
```

The test corpus includes Bootstrap, Gutenberg, Carbon, Angular Components, wp-calypso, Discourse, GOV.UK Frontend, Spectrum CSS, Docusaurus, Grafana, Material UI, freeCodeCamp, PatternFly, Primer CSS, Elastic EUI, Mattermost, Mastodon, JupyterLab, Joomla, SLDS, rsuite, and Fundamental Styles.

### Benchmarks

```bash
bash benchmarks/benchmark.sh         # Full benchmark suite
bash benchmarks/run-benchmark.sh     # Quick benchmark
```

## Releasing

Releases are automated via GitHub Actions when you push a version tag:

```bash
# 1. Update the version in Cargo.toml (workspace.package.version)
# 2. Commit the version bump
# 3. Tag and push
git tag v0.2.0
git push && git push --tags
```

The [release workflow](.github/workflows/release.yml) will:

1. Build binaries for Linux (x64, arm64) and macOS (x64, arm64)
2. Create a GitHub Release with the binaries
3. Publish the npm package (`@lyricalstring/gale`) with the matching version

### Manual npm build

```bash
# Build for current platform
./scripts/build-npm.sh

# Build for all platforms (requires cross + Docker)
./scripts/build-npm.sh --all

# Set npm package version before building
./scripts/build-npm.sh --version 0.2.0
```

## Architecture

Gale is organized as a Cargo workspace with seven crates:

```
gale (binary)
  |
  v
gale_cli         CLI definition (clap), file discovery, orchestration
  |
  +-- gale_config       Config loading, resolution, presets
  +-- gale_linter       Rule trait, registry, runner, 260+ built-in rules
  |     +-- gale_css_parser    CSS/SCSS/Less parser (lightningcss + raffia)
  |     +-- gale_diagnostics   Span, Diagnostic, LintResult, Fix/Edit types
  +-- gale_formatter    Output formatters (text, json, compact)
  +-- gale_lsp          Language Server Protocol server
```

| Crate | Responsibility |
|-------|----------------|
| `gale_css_parser` | Wraps **lightningcss** (CSS) and **raffia** (SCSS/Less) into a unified, owned AST |
| `gale_diagnostics` | Core types: `Span`, `Diagnostic`, `LintResult`, `Fix`, `Edit` |
| `gale_linter` | `Rule` trait, `RuleRegistry`, `LintRunner`, inline disable comments, all rule implementations |
| `gale_config` | Config file discovery, parsing (JSON/YAML/TOML/JS), `extends` resolution, built-in presets |
| `gale_formatter` | `TextFormatter`, `JsonFormatter`, `CompactFormatter` matching Stylelint output |
| `gale_cli` | Clap CLI, file discovery with ignore support, cache layer, `--fix` orchestration |
| `gale_lsp` | LSP server for real-time editor diagnostics |

See [CLAUDE.md](CLAUDE.md) for detailed architecture docs and how to add rules.

## License

[MIT](LICENSE)
