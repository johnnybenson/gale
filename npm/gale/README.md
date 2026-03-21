# Gale

**An extremely fast CSS linter, written in Rust.**

Gale is a drop-in replacement for [Stylelint](https://stylelint.io/) that runs orders of magnitude faster. It reads your existing `.stylelintrc.json`, supports `/* stylelint-disable */` comments, and produces compatible output — so you can switch with zero config changes.

## Features

- **Blazing fast** — Written in Rust with parallel file linting via rayon
- **57 built-in rules** (13 with autofix) — covers stylelint-config-standard core
- **SCSS & Less support** — via raffia parser
- **Stylelint-compatible config** — reads `.stylelintrc.json`, `.stylelintrc.yml`, `gale.json`, `gale.toml`
- **`extends` with presets** — `gale:recommended`, `gale:all`, npm packages, relative paths
- **Autofix** (`--fix`) — automatically fix problems in 13 rules
- **LSP server** (`--lsp`) — real-time diagnostics in any editor
- **VSCode extension** — included at `editors/vscode/`
- **Inline disable comments** — `/* gale-disable */` and `/* stylelint-disable */`
- **File caching** (`--cache`) — skip unchanged files for instant re-lints
- **stdin support** (`--stdin`) — for editor/IDE integration

## Quick Start

### Install via npm

```bash
npm install -D gale-lint
```

### Install via Cargo

```bash
cargo install gale
```

### Initialize and run

```bash
gale --init        # creates gale.json with recommended config
gale .             # lint current directory
gale --fix .       # lint and autofix
```

## CLI Usage

```
gale [OPTIONS] [FILES]...

Arguments:
  [FILES]...                     Files or directories to lint

Options:
      --fix                      Automatically fix problems
  -q, --quiet                    Only report errors
  -f, --formatter <FORMAT>       Output format: text (default), json, compact
  -c, --config <PATH>            Config file path
      --max-warnings <N>         Exit with error if warnings exceed N
      --stdin                    Read source from stdin
      --stdin-filename <NAME>    Virtual filename for stdin [default: stdin.css]
      --ignore-path <FILE>       Custom ignore file (gitignore syntax)
      --no-ignore                Disable all ignore file processing
      --cache                    Enable file caching for unchanged files
      --cache-location <PATH>    Custom cache file path [default: .gale_cache]
      --init                     Generate starter gale.json config
      --print-config <FILE>      Print resolved config for a file as JSON
      --lsp                      Start LSP server for editor integration
  -V, --version                  Print version
  -h, --help                     Print help
```

## Configuration

### gale.json

```json
{
  "extends": "gale:recommended",
  "rules": {
    "declaration-no-important": "error",
    "color-hex-length": "warning",
    "selector-max-id": "off"
  }
}
```

### .stylelintrc.json (compatible)

```json
{
  "extends": "gale:recommended",
  "rules": {
    "block-no-empty": true,
    "color-no-invalid-hex": "error",
    "number-max-precision": ["error", { "max": 4 }]
  }
}
```

### Presets

| Preset | Description |
|--------|-------------|
| `gale:recommended` | 15 error rules + 14 warning rules (sensible defaults) |
| `gale:all` | All rules at warning severity |

### Rule value formats

| Format | Meaning |
|--------|---------|
| `true` | Enable at error severity |
| `false` / `"off"` | Disable |
| `"error"` | Enable at error severity |
| `"warning"` | Enable at warning severity |
| `["error", { options }]` | Enable with options |

## Rules

| Rule | Autofix | Category |
|------|---------|----------|
| `alpha-value-notation` | | Value |
| `annotation-no-unknown` | | Annotation |
| `at-rule-no-unknown` | | At-rule |
| `at-rule-no-vendor-prefix` | Yes | At-rule |
| `block-no-empty` | | Block |
| `color-hex-case` | Yes | Color |
| `color-hex-length` | Yes | Color |
| `color-named` | | Color |
| `color-no-invalid-hex` | | Color |
| `comment-empty-line-before` | | Comment |
| `comment-no-empty` | Yes | Comment |
| `custom-property-no-missing-var-function` | | Custom property |
| `custom-property-pattern` | | Custom property |
| `declaration-block-no-duplicate-custom-properties` | | Declaration block |
| `declaration-block-no-duplicate-properties` | | Declaration block |
| `declaration-block-no-redundant-longhand-properties` | | Declaration block |
| `declaration-block-no-shorthand-property-overrides` | | Declaration block |
| `declaration-empty-line-before` | | Declaration |
| `declaration-no-important` | Yes | Declaration |
| `font-family-no-duplicate-names` | | Font family |
| `font-family-no-missing-generic-family-keyword` | | Font family |
| `function-calc-no-unspaced-operator` | | Function |
| `function-name-case` | Yes | Function |
| `function-url-quotes` | Yes | Function |
| `import-notation` | | Import |
| `keyframe-block-no-duplicate-selectors` | | Keyframe |
| `keyframe-declaration-no-important` | | Keyframe |
| `length-zero-no-unit` | Yes | Length |
| `max-nesting-depth` | | Nesting |
| `media-feature-name-no-unknown` | | Media |
| `media-query-no-invalid` | | Media |
| `no-descending-specificity` | | Specificity |
| `no-duplicate-at-import-rules` | | Import |
| `no-duplicate-selectors` | | Selector |
| `no-empty-source` | | Source |
| `no-invalid-double-slash-comments` | | Comment |
| `no-invalid-position-at-import-rule` | | Import |
| `no-invalid-position-declaration` | | Declaration |
| `no-irregular-whitespace` | | Whitespace |
| `no-unknown-animations` | | Animation |
| `number-max-precision` | | Number |
| `property-no-unknown` | | Property |
| `property-no-vendor-prefix` | Yes | Property |
| `rule-empty-line-before` | | Rule |
| `selector-class-pattern` | | Selector |
| `selector-max-compound-selectors` | | Selector |
| `selector-max-id` | | Selector |
| `selector-no-qualifying-type` | | Selector |
| `selector-pseudo-class-no-unknown` | | Selector |
| `selector-pseudo-element-colon-notation` | Yes | Selector |
| `selector-pseudo-element-no-unknown` | | Selector |
| `selector-type-no-unknown` | | Selector |
| `shorthand-property-no-redundant-values` | Yes | Shorthand |
| `string-no-newline` | | String |
| `unit-no-unknown` | | Unit |
| `value-keyword-case` | Yes | Value |
| `value-no-vendor-prefix` | Yes | Value |

## Inline Disable Comments

```css
/* gale-disable */                        /* disable all rules */
/* gale-enable */                         /* re-enable all rules */
/* gale-disable rule-name */              /* disable specific rule */
/* gale-disable-next-line */              /* disable for next line */
/* gale-disable-next-line rule-name */    /* disable specific rule for next line */

/* stylelint-disable */                   /* also supported for compatibility */
```

## Editor Integration

### VSCode

Install the extension from `editors/vscode/`:

```bash
cd editors/vscode
bun install && bun run compile
```

### Any LSP-compatible editor

```bash
gale --lsp    # starts LSP server on stdin/stdout
```

### stdin (for custom integrations)

```bash
echo "a { color: #ffffff; }" | gale --stdin
echo "a { color: #ffffff; }" | gale --stdin --fix   # outputs fixed CSS to stdout
```

## Migration from Stylelint

1. **Config**: Gale reads `.stylelintrc.json` directly — no changes needed
2. **Comments**: `/* stylelint-disable */` works as-is
3. **Output**: JSON format matches Stylelint's for tooling compatibility
4. **Rules**: Same names, same config format
5. **Quick switch**: `gale --init` then `gale .`

## Performance

Gale is typically **50-100x faster** than Stylelint on large codebases. Run the included benchmark:

```bash
bash benchmarks/run-benchmark.sh
```

## License

MIT
