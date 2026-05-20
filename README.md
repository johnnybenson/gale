# Gale

**An extremely fast CSS linter. Drop-in replacement for Stylelint.**

[![npm version](https://img.shields.io/npm/v/@lyricalstring/gale)](https://www.npmjs.com/package/@lyricalstring/gale)
[![CI](https://github.com/LyricalString/gale/actions/workflows/ci.yml/badge.svg)](https://github.com/LyricalString/gale/actions)
[![license: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Gale reads your existing `.stylelintrc`, runs the same rules, and produces the same output. Just **10x-100x faster**.

One line change in your `package.json`. No config migration.

> **Compatibility note:** Gale targets **Stylelint v17** semantics. If your project uses Stylelint v16 or earlier, you may see minor differences in edge cases (e.g., how `selector-max-*` rules count selectors inside `:is()`, `:has()`, `:where()`). These match the behavior you would get after upgrading to Stylelint v17.

## Benchmarks

Real-world benchmarks using [hyperfine](https://github.com/sharkdp/hyperfine) (10 runs, 3 warmup) on an Apple M4 Max. Each repo uses its own Stylelint config. Results vary by machine -- run `./benchmarks/benchmark.sh` to reproduce on yours.

| Repository | Files | Stylelint | Gale | Speedup |
|------------|------:|----------:|-----:|--------:|
| [Angular Components](https://github.com/angular/components) | 621 | 0.743s | 0.008s | **96x** |
| [Fundamental Styles](https://github.com/SAP/fundamental-styles) | 392 | 3.628s | 0.060s | **61x** |
| [GOV.UK Frontend](https://github.com/alphagov/govuk-frontend) | 149 | 2.352s | 0.044s | **54x** |
| [Discourse](https://github.com/discourse/discourse) | 356 | 0.529s | 0.010s | **51x** |
| [Joomla](https://github.com/joomla/joomla-cms) | 169 | 1.033s | 0.025s | **41x** |
| [Carbon](https://github.com/carbon-design-system/carbon) | 1,116 | 0.385s | 0.010s | **38x** |
| [Bootstrap](https://github.com/twbs/bootstrap) | 99 | 0.737s | 0.021s | **35x** |
| [Gutenberg](https://github.com/wordpress/gutenberg) | 778 | 0.447s | 0.013s | **34x** |
| [PatternFly](https://github.com/patternfly/patternfly) | 204 | 0.377s | 0.013s | **29x** |
| [SLDS](https://github.com/salesforce-ux/design-system) | 446 | 0.323s | 0.014s | **24x** |

**Parity: 0 false positives and 0 false negatives across 22 tested repositories (5,790+ files).**

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

The npm package includes prebuilt binaries for supported platforms, so install
does not run a postinstall script or download executables. Supported platforms:
macOS (arm64, x64), Linux (x64, arm64).

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

### 270+ built-in rules

Gale ships 270+ built-in rules across five categories:

| Category | Count | Examples |
|----------|------:|---------|
| Core Stylelint | 146 | `block-no-empty`, `color-no-invalid-hex`, `property-no-unknown`, `display-notation` |
| SCSS (`scss/*`) | 44 | `scss/at-rule-no-unknown`, `scss/no-duplicate-mixins`, `scss/dollar-variable-pattern` |
| Stylistic (`@stylistic/*`) | 59 | `stylistic/indentation`, `stylistic/declaration-colon-space-after`, `stylistic/no-eol-whitespace` |
| Order (`order/*`) | 3 | `order/order`, `order/properties-order`, `order/properties-alphabetical-order` |
| Plugin (`plugin/*`) | 4 | `plugin/enforce-variable-for-property`, `plugin/no-unknown-custom-properties`, `plugin/no-unused-custom-properties`, `plugin/require-file-header-comment` |

SCSS, stylistic, and plugin rules are built in -- no extra plugins required.

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
- **JSON, text, compact, verbose, TAP, and unix** output formatters matching Stylelint's format
- **Custom JS formatters** via `--custom-formatter`
- **Programmatic Node.js API** (`lint()`, `resolveConfig()`, `formatters`) compatible with `stylelint.lint()`
- **`extends`** with built-in presets, npm packages, and relative paths
- **`.galeignore`** files (gitignore syntax) for custom exclusions

### Declarative plugin rules

Gale includes 4 built-in plugin meta-rules that cover the most common custom plugin patterns (design token enforcement, custom property analysis, file header checks). These replace the need for JS plugins like `stylelint-plugin-carbon-tokens`, Primer's custom plugins, and `stylelint-copyright`:

| Rule | Description |
|------|-------------|
| `plugin/enforce-variable-for-property` | Enforce design token/variable usage for configured properties |
| `plugin/no-unknown-custom-properties` | Report usage of undefined CSS custom properties |
| `plugin/no-unused-custom-properties` | Report defined but unused CSS custom properties |
| `plugin/require-file-header-comment` | Require a file header comment matching a pattern |

### Programmatic API

```javascript
import { lint, resolveConfig, formatters } from '@lyricalstring/gale';

const result = await lint({
  files: 'src/**/*.css',
  config: { rules: { 'block-no-empty': true } },
});

console.log(result.errored);        // boolean
console.log(result.results);        // LintResult[]
console.log(result.report);         // formatted string
```

### Not yet supported

- **Arbitrary JavaScript plugins.** Gale cannot execute JS plugins, but its 270+ built-in rules and 4 plugin meta-rules cover the vast majority of real-world configs. See [Declarative plugin rules](#declarative-plugin-rules) above.
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
| `--fix` | Automatically fix problems (default: strict mode) |
| `--fix=lax` | Fix problems even in files with parse errors |
| `-q, --quiet` | Only report errors |
| `-f, --formatter <type>` | Output: `text` (default), `string`, `json`, `compact`, `verbose`, `tap`, `unix` |
| `--custom-formatter <module>` | Path or npm package name for a custom JS formatter |
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
3. Stage those binaries inside the npm package
4. Publish the npm package (`@lyricalstring/gale`) with the matching version

### Manual npm build

```bash
# Build and stage the current platform binary in npm/bin/<target>/gale
./scripts/build-npm.sh

# Build and stage all supported binaries (requires cross + Docker)
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
  +-- gale_formatter    Output formatters (text, json, compact, verbose, tap, unix)
  +-- gale_lsp          Language Server Protocol server
```

| Crate | Responsibility |
|-------|----------------|
| `gale_css_parser` | Wraps **lightningcss** (CSS) and **raffia** (SCSS/Less) into a unified, owned AST |
| `gale_diagnostics` | Core types: `Span`, `Diagnostic`, `LintResult`, `Fix`, `Edit` |
| `gale_linter` | `Rule` trait, `RuleRegistry`, `LintRunner`, inline disable comments, all rule implementations |
| `gale_config` | Config file discovery, parsing (JSON/YAML/TOML/JS), `extends` resolution, built-in presets |
| `gale_formatter` | `TextFormatter`, `JsonFormatter`, `CompactFormatter`, `VerboseFormatter`, `TapFormatter`, `UnixFormatter` matching Stylelint output |
| `gale_cli` | Clap CLI, file discovery with ignore support, cache layer, `--fix` orchestration |
| `gale_lsp` | LSP server for real-time editor diagnostics |

See [CLAUDE.md](CLAUDE.md) for detailed architecture docs and how to add rules.

## License

[MIT](LICENSE)
