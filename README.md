# Gale

**An extremely fast CSS linter. Drop-in replacement for Stylelint.**

<!--
[![npm version](https://img.shields.io/npm/v/gale-lint)](https://www.npmjs.com/package/gale-lint)
[![downloads](https://img.shields.io/npm/dm/gale-lint)](https://www.npmjs.com/package/gale-lint)
[![CI](https://github.com/LyricalString/gale/actions/workflows/ci.yml/badge.svg)](https://github.com/LyricalString/gale/actions)
[![license: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
-->

Gale is a CSS linter written in Rust that reads your existing `.stylelintrc`, runs the same rules, and produces the same output -- just **4.9x faster**. One line change in your `package.json`. No config migration.

## Highlights

- **4.9x faster** than Stylelint on real-world projects (Bootstrap: 2.36s vs 0.48s)
- **True drop-in** -- reads `.stylelintrc.json`, `.stylelintrc.yml`, and all standard config formats
- **0 false positives** on Bootstrap (99/99 SCSS files produce identical output)
- **57 built-in rules** (13 with autofix) covering the Stylelint core rule set
- **SCSS and Less** support out of the box
- **Autofix** via `--fix`
- **File caching** via `--cache` for instant re-runs on unchanged files
- **LSP server** for real-time diagnostics in any editor
- **Parallel linting** -- uses all available CPU cores via rayon

## Quick Start

```bash
# Install
npm install -D gale-lint

# Lint (uses your existing .stylelintrc)
npx gale "src/**/*.css"

# Or with autofix
npx gale --fix "src/**/*.css"
```

To switch from Stylelint, change one line in `package.json`:

```diff
 {
   "scripts": {
-    "lint:css": "stylelint 'src/**/*.css'"
+    "lint:css": "gale 'src/**/*.css'"
   }
 }
```

Your `.stylelintrc` stays exactly the same.

## Benchmark

Tested on [Bootstrap](https://github.com/twbs/bootstrap) (99 SCSS files) using the project's own Stylelint configuration:

| Tool | Time | Result |
|------|------|--------|
| Stylelint | 2.36s | baseline |
| **Gale** | **0.48s** | **4.9x faster** |

**Parity: 99/99 files match.** Zero false positives. Zero false negatives. Identical output.

Benchmarks are fully reproducible -- see [`benchmarks/`](benchmarks/) and the [differential testing harness](tests/differential/).

## Migrate from Stylelint

Three steps:

1. **Install:**
   ```bash
   npm install -D gale-lint
   ```

2. **Replace `stylelint` with `gale` in your scripts:**
   ```bash
   npx gale "src/**/*.css"
   ```

3. **Done.** Your `.stylelintrc` works as-is.

Gale reads your existing config, follows the same `extends` chains, honors `/* stylelint-disable */` comments, and produces the same JSON output format for CI tooling.

## Configuration

### Supported config files

Gale searches for config files walking up from the working directory, in this order:

| File | Format |
|------|--------|
| `gale.json` | JSON |
| `gale.toml` | TOML |
| `.stylelintrc` | JSON or YAML |
| `.stylelintrc.json` | JSON |
| `.stylelintrc.yml` | YAML |
| `.stylelintrc.yaml` | YAML |

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
  },
  "ignorePatterns": ["dist/**", "vendor/**"]
}
```

### Rule severity values

| Value | Effect |
|-------|--------|
| `true` or `"error"` | Enable at error severity |
| `"warning"` | Enable at warning severity |
| `false` or `"off"` | Disable the rule |
| `["error", { options }]` | Enable with rule-specific options |

### Built-in presets

| Preset | Description |
|--------|-------------|
| `gale:recommended` | 29 rules (15 errors + 14 warnings) -- sensible defaults |
| `gale:all` | All 57 rules enabled at warning severity |

### Extends

The `extends` field supports multiple sources:

| Value | Resolution |
|-------|------------|
| `"gale:recommended"` | Built-in preset |
| `"gale:all"` | Built-in preset |
| `"./path/to/config.json"` | Relative file path |
| `"stylelint-config-standard"` | npm package (resolved from `node_modules/`) |

Resolution is recursive with cycle detection. User `rules` always override extended rules.

### Inline disable comments

Both `gale-` and `stylelint-` prefixes are supported:

```css
/* gale-disable */                        /* disable all rules */
/* gale-enable */                         /* re-enable all rules */
/* gale-disable rule-name */              /* disable specific rule */
/* gale-disable-next-line rule-name */    /* disable specific rule for next line */

/* stylelint-disable */                   /* also works (compatibility) */
```

## Rules

57 built-in rules, organized by category. Rules marked with a wrench support `--fix`.

### At-Rule

| Rule | Fix | Description |
|------|-----|-------------|
| `at-rule-no-unknown` | | Disallow unknown at-rules |
| `at-rule-no-vendor-prefix` | Yes | Disallow vendor prefixes for at-rules |

### Annotation

| Rule | Fix | Description |
|------|-----|-------------|
| `annotation-no-unknown` | | Disallow unknown annotations |

### Block

| Rule | Fix | Description |
|------|-----|-------------|
| `block-no-empty` | | Disallow empty blocks |

### Color

| Rule | Fix | Description |
|------|-----|-------------|
| `color-hex-case` | Yes | Specify lowercase or uppercase for hex colors |
| `color-hex-length` | Yes | Specify short or long notation for hex colors |
| `color-named` | | Require or disallow named colors |
| `color-no-invalid-hex` | | Disallow invalid hex colors |

### Comment

| Rule | Fix | Description |
|------|-----|-------------|
| `comment-empty-line-before` | | Require or disallow an empty line before comments |
| `comment-no-empty` | Yes | Disallow empty comments |
| `no-invalid-double-slash-comments` | | Disallow `//` comments in CSS |

### Custom Property

| Rule | Fix | Description |
|------|-----|-------------|
| `custom-property-no-missing-var-function` | | Disallow missing `var()` for custom properties |
| `custom-property-pattern` | | Specify a pattern for custom properties |

### Declaration

| Rule | Fix | Description |
|------|-----|-------------|
| `declaration-empty-line-before` | | Require or disallow an empty line before declarations |
| `declaration-no-important` | Yes | Disallow `!important` within declarations |
| `declaration-block-no-duplicate-custom-properties` | | Disallow duplicate custom properties in a block |
| `declaration-block-no-duplicate-properties` | | Disallow duplicate properties in a block |
| `declaration-block-no-redundant-longhand-properties` | | Disallow longhand properties that can be combined |
| `declaration-block-no-shorthand-property-overrides` | | Disallow shorthand properties that override longhands |
| `no-invalid-position-declaration` | | Disallow declarations in invalid positions |

### Font Family

| Rule | Fix | Description |
|------|-----|-------------|
| `font-family-no-duplicate-names` | | Disallow duplicate font family names |
| `font-family-no-missing-generic-family-keyword` | | Disallow a missing generic family keyword |

### Function

| Rule | Fix | Description |
|------|-----|-------------|
| `function-calc-no-unspaced-operator` | | Disallow unspaced operators in `calc()` |
| `function-name-case` | Yes | Specify lowercase for function names |
| `function-url-quotes` | Yes | Require quotes for URLs |

### Import

| Rule | Fix | Description |
|------|-----|-------------|
| `import-notation` | | Specify string or URL notation for `@import` |
| `no-duplicate-at-import-rules` | | Disallow duplicate `@import` rules |
| `no-invalid-position-at-import-rule` | | Disallow `@import` in invalid positions |

### Keyframe

| Rule | Fix | Description |
|------|-----|-------------|
| `keyframe-block-no-duplicate-selectors` | | Disallow duplicate selectors in keyframe blocks |
| `keyframe-declaration-no-important` | | Disallow `!important` in keyframe declarations |

### Media

| Rule | Fix | Description |
|------|-----|-------------|
| `media-feature-name-no-unknown` | | Disallow unknown media feature names |
| `media-query-no-invalid` | | Disallow invalid media queries |

### Number and Length

| Rule | Fix | Description |
|------|-----|-------------|
| `alpha-value-notation` | | Specify percentage or number notation for alpha values |
| `length-zero-no-unit` | Yes | Disallow units for zero lengths |
| `number-max-precision` | | Limit the number of decimal places |

### Property

| Rule | Fix | Description |
|------|-----|-------------|
| `property-no-unknown` | | Disallow unknown properties |
| `property-no-vendor-prefix` | Yes | Disallow vendor prefixes for properties |
| `shorthand-property-no-redundant-values` | Yes | Disallow redundant values in shorthand properties |

### Selector

| Rule | Fix | Description |
|------|-----|-------------|
| `no-descending-specificity` | | Disallow lower specificity selectors after higher ones |
| `no-duplicate-selectors` | | Disallow duplicate selectors |
| `selector-class-pattern` | | Specify a pattern for class selectors |
| `selector-max-compound-selectors` | | Limit compound selectors |
| `selector-max-id` | | Limit ID selectors |
| `selector-no-qualifying-type` | | Disallow qualifying a selector by type |
| `selector-pseudo-class-no-unknown` | | Disallow unknown pseudo-class selectors |
| `selector-pseudo-element-colon-notation` | Yes | Specify `::` notation for pseudo-elements |
| `selector-pseudo-element-no-unknown` | | Disallow unknown pseudo-element selectors |
| `selector-type-no-unknown` | | Disallow unknown type selectors |

### Source and Whitespace

| Rule | Fix | Description |
|------|-----|-------------|
| `no-empty-source` | | Disallow empty sources |
| `no-irregular-whitespace` | | Disallow irregular whitespace |

### String

| Rule | Fix | Description |
|------|-----|-------------|
| `string-no-newline` | | Disallow newlines in strings |

### Unit

| Rule | Fix | Description |
|------|-----|-------------|
| `unit-no-unknown` | | Disallow unknown units |

### Value

| Rule | Fix | Description |
|------|-----|-------------|
| `value-keyword-case` | Yes | Specify lowercase for keyword values |
| `value-no-vendor-prefix` | Yes | Disallow vendor prefixes for values |

### Animation

| Rule | Fix | Description |
|------|-----|-------------|
| `no-unknown-animations` | | Disallow unknown animations |

### Nesting

| Rule | Fix | Description |
|------|-----|-------------|
| `max-nesting-depth` | | Limit nesting depth |

### Other

| Rule | Fix | Description |
|------|-----|-------------|
| `rule-empty-line-before` | | Require or disallow an empty line before rules |

## CLI Reference

```
gale [OPTIONS] [FILES]...
```

| Flag | Description |
|------|-------------|
| `<files>` | Files, directories, or glob patterns to lint |
| `--fix` | Automatically fix problems where possible |
| `-q, --quiet` | Only report errors, suppress warnings |
| `-f, --formatter <type>` | Output format: `text` (default), `json`, `compact` |
| `-c, --config <path>` | Path to config file (auto-detected if omitted) |
| `--max-warnings <n>` | Exit with error if warning count exceeds threshold |
| `--stdin` | Read CSS from standard input |
| `--stdin-filename <name>` | Filename for stdin input (default: `stdin.css`) |
| `--ignore-path <file>` | Custom ignore file (gitignore syntax) |
| `--no-ignore` | Disable ignore file processing |
| `--cache` | Cache results to skip unchanged files |
| `--cache-location <path>` | Cache file location (default: `.gale_cache`) |
| `--init` | Generate a starter `gale.json` config |
| `--print-config <file>` | Print resolved config for a file as JSON |
| `--lsp` | Start LSP server for editor integration |

## Editor Integration

### VS Code

A VS Code extension is included at [`editors/vscode/`](editors/vscode/):

```bash
cd editors/vscode
bun install && bun run compile
```

### Any LSP-compatible editor

```bash
gale --lsp
```

Gale starts an LSP server on stdin/stdout, compatible with Neovim, Helix, Zed, and any editor that supports the Language Server Protocol.

### stdin

```bash
echo "a { color: #fff; }" | gale --stdin
echo "a { color: #fff; }" | gale --stdin --fix
```

## Differences from Stylelint

Gale aims for full output compatibility with Stylelint. Here is what works and what does not -- yet.

**Fully supported:**
- 57 core Stylelint rules with identical behavior
- All config formats (`.stylelintrc.json`, `.stylelintrc.yml`, `.stylelintrc.yaml`, `.stylelintrc`)
- Native config formats (`gale.json`, `gale.toml`)
- `extends` with built-in presets, npm packages, and relative paths
- Config overrides
- Inline disable comments (`stylelint-disable` and `gale-disable`)
- JSON, text, and compact output formatters matching Stylelint's format
- Exit codes (0 for clean, 1 for errors)
- SCSS and Less file support

**Not yet supported:**
- **Custom JavaScript plugins.** Third-party rule packages like `@stylistic/*`, `stylelint-scss/*`, and community plugins are not supported. Gale only runs its built-in rules.
- **`package.json` config.** The `"stylelint"` field in `package.json` (cosmiconfig-style) is not read.
- **Sass indented syntax.** `.sass` files are not supported (`.scss` files work fine).
- **Some rule options.** A few rarely-used rule options (e.g., `ignoreAtRules` for `at-rule-no-unknown`) are not yet implemented.

If you find a case where Gale produces different output than Stylelint on the same config, please [open an issue](https://github.com/LyricalString/gale/issues). Compatibility is a top priority.

## Contributing

Contributions are welcome. Gale is a Rust workspace with a modular crate structure:

```
gale/
  src/main.rs               Binary entrypoint
  crates/
    gale_cli/                CLI, file discovery, caching
    gale_config/             Config loading, extends resolution, presets
    gale_css_parser/         CSS/SCSS/Less parser (lightningcss + raffia)
    gale_diagnostics/        Diagnostic types, spans, autofix
    gale_formatter/          Output formatters (text, json, compact)
    gale_linter/             Rule trait, registry, 57 built-in rules
    gale_lsp/                LSP server
  benchmarks/                Benchmark scripts vs Stylelint
  tests/differential/        Parity testing against Stylelint on real repos
```

```bash
cargo build                          # build
cargo test --workspace               # run all tests
cargo test -p gale_linter            # test a specific crate
cargo run -- "src/**/*.css"          # run the linter
cargo run -- --fix "src/**/*.css"    # run with autofix
```

See [CLAUDE.md](CLAUDE.md) for detailed architecture documentation, including how to add new rules.

## License

[MIT](LICENSE)
