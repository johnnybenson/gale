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
| [Primer CSS](https://github.com/primer/css) | 113 | 1.285s | 0.010s | **129x** |
| [Gutenberg](https://github.com/wordpress/gutenberg) | 775 | 4.715s | 0.042s | **112x** |
| [wp-calypso](https://github.com/Automattic/wp-calypso) | 2,238 | 13.223s | 0.131s | **101x** |
| [GOV.UK Frontend](https://github.com/alphagov/govuk-frontend) | 163 | 1.614s | 0.011s | **147x** |
| [Discourse](https://github.com/discourse/discourse) | 355 | 1.438s | 0.021s | **69x** |
| [Angular Components](https://github.com/angular/components) | 620 | 1.843s | 0.021s | **88x** |

**Parity: 0 false positives and 0 false negatives across all 16 tested repositories (6,673 files).**

Reproduce these results: `./benchmarks/benchmark.sh`

## Quick Start

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

## What's supported

- **250+ built-in rules** including core Stylelint, SCSS, and @stylistic rules
- **All config formats**: `.stylelintrc.json`, `.stylelintrc.yml`, `.stylelintrc.yaml`, `.stylelintrc`, `stylelint.config.js`, `stylelint.config.cjs`
- **`extends`** with built-in presets, npm packages, and relative paths
- **SCSS and Less** out of the box
- **Autofix** via `--fix`
- **File caching** via `--cache`
- **LSP server** for editor integration
- **Parallel linting** using all CPU cores
- **Inline disable comments** (`stylelint-disable` and `gale-disable`)
- **JSON, text, and compact** output formatters matching Stylelint's format

### Not yet supported

- **Custom JavaScript plugins.** Third-party rule packages (community plugins) are not supported. Gale only runs its built-in rules.
- **`package.json` config.** The `"stylelint"` field in `package.json` is not read.
- **Sass indented syntax.** `.sass` files are not supported (`.scss` works fine).

## Configuration

Gale searches for config files walking up from the working directory:

| File | Format |
|------|--------|
| `gale.json` | JSON |
| `gale.toml` | TOML |
| `.stylelintrc` | JSON or YAML |
| `.stylelintrc.json` | JSON |
| `.stylelintrc.yml` / `.yaml` | YAML |
| `stylelint.config.js` / `.cjs` | JavaScript |

Or generate a starter config:

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

### Built-in presets

| Preset | Description |
|--------|-------------|
| `gale:recommended` | Sensible defaults (29 rules) |
| `gale:all` | All rules enabled at warning severity |

You can also extend npm packages like `stylelint-config-standard` directly.

## CLI Reference

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
| `--lsp` | Start LSP server |
| `--stdin` | Read from stdin |
| `--print-config <file>` | Print resolved config as JSON |
| `--init` | Generate starter config |

## Editor Integration

### VS Code

Extension included at [`editors/vscode/`](editors/vscode/).

### Any LSP-compatible editor

```bash
gale --lsp
```

Works with Neovim, Helix, Zed, and any editor supporting the Language Server Protocol.

## Contributing

```bash
cargo build                          # build
cargo test --workspace               # run all tests
cargo run -- "src/**/*.css"          # lint
cargo run -- --fix "src/**/*.css"    # autofix
```

See [CLAUDE.md](CLAUDE.md) for architecture docs and how to add rules.

## License

[MIT](LICENSE)
