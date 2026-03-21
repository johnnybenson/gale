# Gale -- An extremely fast CSS linter, written in Rust

## Project overview

Gale is a **drop-in replacement for [Stylelint](https://stylelint.io/)**, written in Rust for maximum performance. The goal is 100% compatibility with Stylelint's rule semantics, config format, CLI flags, and output -- so teams can switch from Stylelint to Gale with zero config changes.

- Reads `.stylelintrc.json`, `.stylelintrc.yml`, `.stylelintrc` (JSON or YAML), `gale.json`, `gale.toml`
- Supports `extends` (built-in presets + npm packages + relative paths)
- Matches Stylelint's JSON output format for tooling integration
- Supports `/* stylelint-disable */` and `/* gale-disable */` inline comments
- Parallel file linting via `rayon`
- Auto-fix support (`--fix`)
- File caching (`--cache`)
- LSP server for editor integration (`--lsp`)

---

## Architecture

### Crate dependency graph

```
src/main.rs  (binary entrypoint -- delegates to gale_cli::run())
    |
    v
gale_cli        CLI definition (clap), file discovery, orchestration
    |
    +-- gale_config       Config loading, resolution, presets
    +-- gale_linter       Rule trait, registry, runner, built-in rules
    |       +-- gale_css_parser    CSS/SCSS/Less parser wrapper
    |       +-- gale_diagnostics   Span, Diagnostic, LintResult, Fix/Edit types
    +-- gale_formatter    Output formatters (text, json, compact)
    +-- gale_lsp          Language Server Protocol server
```

### Data flow

```
CLI args
  -> resolve config (find_config -> load_config -> resolve extends/presets)
  -> discover CSS files (ignore + WalkBuilder)
  -> for each file (in parallel via rayon):
       parse(source, syntax) -> ParseResult { nodes: Vec<CssNode> }
       for each enabled rule:
         rule.check_root(nodes, context) -> Vec<Diagnostic>
         walk AST: rule.check(node, context) -> Vec<Diagnostic>
       filter by inline disable comments
       sort diagnostics by offset
  -> apply fixes if --fix
  -> format output (text/json/compact)
  -> exit code 1 if errors found
```

### Crate-by-crate breakdown

| Crate | Path | Responsibility |
|-------|------|----------------|
| **gale_css_parser** | `crates/gale_css_parser/` | Wraps **lightningcss** (CSS) and **raffia** (SCSS/Less) into a unified, lifetime-free AST (`CssNode` enum). Handles syntax detection from file extensions. |
| **gale_diagnostics** | `crates/gale_diagnostics/` | Core types: `Span`, `SourceLocation`, `SourceLineIndex`, `Severity`, `Diagnostic`, `LintResult`, `Fix`, `Edit`. Also provides `apply_fixes()`. |
| **gale_linter** | `crates/gale_linter/` | The `Rule` trait, `RuleRegistry`, `LintRunner`, inline disable-comment processing, known-identifier data tables, and all 50+ built-in rule implementations. |
| **gale_config** | `crates/gale_config/` | Config file discovery (walks up directories), parsing (JSON/YAML/TOML), `extends` resolution (built-in presets, npm packages, relative paths), preset definitions (`gale:recommended`, `gale:all`). |
| **gale_formatter** | `crates/gale_formatter/` | `Formatter` trait with `TextFormatter` (Stylelint-like), `JsonFormatter` (Stylelint-compatible JSON), and `CompactFormatter`. Factory function `create_formatter()`. |
| **gale_cli** | `crates/gale_cli/` | Clap-based CLI, file discovery with `.galeignore`/`.gitignore` support, cache layer, `--fix` orchestration, `--init` scaffolding, `--lsp` delegation. |
| **gale_lsp** | `crates/gale_lsp/` | LSP server for editor integration. Invoked via `gale --lsp`. |

---

## Key types and traits

### `Rule` trait (`gale_linter::rule`)

```rust
pub trait Rule: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn default_severity(&self) -> Severity;

    // Per-node check (called for every AST node during tree walk)
    fn check(&self, node: &CssNode, context: &RuleContext) -> Vec<Diagnostic>;

    // Document-level check (called once with all top-level nodes)
    fn check_root(&self, nodes: &[CssNode], context: &RuleContext) -> Vec<Diagnostic>;
}
```

Most rules implement `check()` for per-node inspection. Rules that need cross-node context (e.g., duplicate detection, source-level scans) implement `check_root()` instead.

### `RuleContext`

```rust
pub struct RuleContext<'a> {
    pub file_path: &'a str,
    pub source: &'a str,
    pub syntax: Syntax,
}
```

### `CssNode` (simplified AST)

```rust
pub enum CssNode {
    Style(StyleRule),    // selector + declarations + nested children
    AtRule(AtRule),       // @media, @keyframes, etc. + children
    Comment(Comment),    // /* text */
    Declaration(Declaration),  // property: value
}
```

All AST types are **owned** (no lifetimes) and `Serialize`/`Deserialize`. Each has a `Span { offset, length }` for byte-offset positions.

### `Diagnostic`

```rust
pub struct Diagnostic {
    pub rule_name: String,
    pub message: String,
    pub severity: Severity,     // Error, Warning, Info, Hint
    pub span: Span,             // byte offset + length
    pub file_path: String,
    pub fix: Option<Fix>,       // optional auto-fix
}
```

Builder pattern: `Diagnostic::new(rule, msg).severity(s).span(sp).fix(f)`

### `LintResult`

```rust
pub struct LintResult {
    pub file_path: String,
    pub diagnostics: Vec<Diagnostic>,
    pub source: String,
}
```

### `GaleConfig`

```rust
pub struct GaleConfig {
    pub rules: HashMap<String, RuleConfig>,
    pub ignore_patterns: Vec<String>,
    pub formatter: FormatterType,
}
```

### `RuleConfig`

```rust
pub struct RuleConfig {
    pub severity: Option<Severity>,
    pub options: Option<serde_json::Value>,
}
```

### `RuleRegistry` and `LintRunner`

- `RuleRegistry` -- stores `Vec<Box<dyn Rule>>`, created via `RuleRegistry::default()` (calls `register_all()`).
- `LintRunner` -- holds a registry + list of enabled rule names. `lint_source(source, path, syntax) -> LintResult`.

---

## Build and test commands

```sh
# Build (debug)
cargo build

# Build (release)
cargo build --release

# Run all tests
cargo test --workspace

# Run tests for a specific crate
cargo test -p gale_linter
cargo test -p gale_config

# Run a specific test
cargo test -p gale_linter block_no_empty

# Benchmark (uses hyperfine if available, falls back to `time`)
bash benchmarks/run-benchmark.sh

# Differential testing against Stylelint
python tests/differential/run.py                    # all repos
python tests/differential/run.py bootstrap          # specific repo
python tests/differential/run.py --benchmark        # with timing
python tests/differential/run.py bootstrap --css-only  # skip SCSS/Less

# Run the linter
cargo run -- src/              # lint a directory
cargo run -- file.css          # lint a file
cargo run -- --fix src/        # auto-fix
cargo run -- --formatter json src/  # JSON output

# Debug performance (per-phase timings to stderr)
GALE_DEBUG_PERF=1 cargo run --release -- src/

# Tracing/logging
GALE_LOG=debug cargo run -- src/
```

---

## How to add a new rule

### Step 1: Create the rule file

Create `crates/gale_linter/src/rules/your_rule_name.rs`:

```rust
use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

pub struct YourRuleName;

impl Rule for YourRuleName {
    fn name(&self) -> &'static str {
        "your-rule-name"  // kebab-case, matching Stylelint's name
    }

    fn description(&self) -> &'static str {
        "Disallow something bad"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning  // or Severity::Error
    }

    fn check(&self, node: &CssNode, _context: &RuleContext) -> Vec<Diagnostic> {
        match node {
            CssNode::Style(rule) => {
                // Inspect the node, return diagnostics
                vec![]
            }
            _ => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{Syntax, /* other types as needed */};

    fn ctx() -> RuleContext<'static> {
        RuleContext { file_path: "test.css", source: "", syntax: Syntax::Css }
    }

    #[test]
    fn test_detects_problem() {
        let rule = YourRuleName;
        // construct a CssNode, call rule.check(), assert diagnostics
    }
}
```

### Step 2: Register the module

Add to `crates/gale_linter/src/rules/mod.rs`:

```rust
pub mod your_rule_name;
```

### Step 3: Register in `register_all()`

In the same file (`mod.rs`), add to the `register_all()` function:

```rust
registry.register(Box::new(your_rule_name::YourRuleName));
```

### Step 4 (if applicable): Add to config presets

If the rule should be part of `gale:recommended` or `gale:all`, add the rule name to the appropriate constant in `crates/gale_config/src/lib.rs`:

- `ALL_RULE_NAMES` -- must be updated for any new rule
- `RECOMMENDED_ERROR_RULES` or `RECOMMENDED_WARNING_RULES` -- if the rule should be in `gale:recommended`

### Per-node vs. document-level rules

- **`check(node, ctx)`** -- called for every node during AST walk. Use for rules that inspect individual declarations, selectors, or at-rules (e.g., `block-no-empty`, `color-no-invalid-hex`).
- **`check_root(nodes, ctx)`** -- called once with all top-level nodes. Use for rules that need cross-node context like duplicate detection (e.g., `no-duplicate-selectors`, `no-empty-source`).

### Rules with auto-fix

Return a `Fix` on the `Diagnostic`:

```rust
use gale_diagnostics::{Edit, Fix};

Diagnostic::new(self.name(), "message")
    .span(span)
    .fix(Fix::new("description", vec![
        Edit::new(Span::new(offset, length), "replacement text"),
    ]))
```

---

## Config system

### Config file search order

`find_config()` walks up from the working directory, checking these names in order:

1. `gale.json`
2. `gale.toml`
3. `.stylelintrc`
4. `.stylelintrc.json`
5. `.stylelintrc.yml`
6. `.stylelintrc.yaml`

### Config file formats

All formats support the same fields:

```json
{
  "extends": "gale:recommended",
  "rules": {
    "block-no-empty": true,
    "color-hex-length": "warning",
    "number-max-precision": ["error", { "max": 4 }],
    "declaration-no-important": "off"
  },
  "ignorePatterns": ["dist/**", "vendor/**"],
  "formatter": "text"
}
```

**Rule value formats** (matching Stylelint):

| Format | Meaning |
|--------|---------|
| `true` | Enable at error severity |
| `false` | Disable |
| `"error"` | Enable at error severity |
| `"warning"` | Enable at warning severity |
| `"off"` | Disable |
| `["error", { options }]` | Enable with options |

### Extends resolution

The `extends` field supports:

| Value | Resolution |
|-------|------------|
| `"gale:recommended"` | Built-in preset: 15 error rules + 14 warning rules |
| `"gale:all"` | Built-in preset: all rules at warning severity |
| `"./path/to/config.json"` | Relative path to another config file |
| `"stylelint-config-standard"` | npm package (looks in `node_modules/`) |

Resolution is recursive with cycle detection. Later `extends` entries override earlier ones. User `rules` always override extended rules.

### Built-in presets

**`gale:recommended`** enables:

- **Error rules** (15): `block-no-empty`, `color-no-invalid-hex`, `declaration-block-no-duplicate-properties`, `declaration-block-no-duplicate-custom-properties`, `font-family-no-duplicate-names`, `no-duplicate-at-import-rules`, `no-duplicate-selectors`, `no-empty-source`, `property-no-unknown`, `selector-pseudo-class-no-unknown`, `selector-pseudo-element-no-unknown`, `selector-type-no-unknown`, `unit-no-unknown`, `no-descending-specificity`, `keyframe-block-no-duplicate-selectors`

- **Warning rules** (14): `color-hex-length`, `color-hex-case`, `length-zero-no-unit`, `declaration-no-important`, `selector-pseudo-element-colon-notation`, `no-invalid-double-slash-comments`, `function-name-case`, `shorthand-property-no-redundant-values`, `at-rule-no-vendor-prefix`, `property-no-vendor-prefix`, `value-no-vendor-prefix`, `value-keyword-case`, `function-url-quotes`, `number-max-precision`

**`gale:all`** enables all rules at warning severity.

---

## Inline disable comments

Both `gale-` and `stylelint-` prefixes are supported:

```css
/* gale-disable */            /* disable all rules until enable */
/* gale-enable */             /* re-enable all rules */
/* gale-disable rule-name */  /* disable specific rule */
/* gale-enable rule-name */   /* re-enable specific rule */
/* gale-disable-next-line */  /* disable all rules on the next line only */
/* gale-disable-next-line rule-name */  /* disable specific rule on next line */

/* stylelint-disable */       /* also works (compatibility) */
```

---

## Differential testing

The differential testing harness (`tests/differential/`) validates Gale as a drop-in replacement by comparing output against Stylelint on real-world repositories.

### Quick start

```bash
python tests/differential/run.py              # all repos
python tests/differential/run.py bootstrap    # specific repo
python tests/differential/run.py --list       # list available repos
python tests/differential/run.py --benchmark  # include timing comparison
python tests/differential/run.py --css-only   # skip SCSS/Less files
python tests/differential/run.py --skip-build # use existing binary
```

### How it works

1. Shallow-clones repos from `tests/differential/repos.json`
2. Installs npm deps (auto-detects npm/yarn/pnpm/bun)
3. Runs both Stylelint and Gale with JSON output using the repo's own config
4. Normalizes output and compares by `(line, column, rule, severity, text)`
5. Reports parity score, per-rule FN/FP breakdown, and sample diffs

### Test corpus

| Name | Repo | Notes |
|------|------|-------|
| grafana | grafana/grafana | Simple config, mostly disabled rules |
| bootstrap | twbs/bootstrap | Industry standard, mature SCSS config |
| gutenberg | wordpress/gutenberg | Mix of legacy and modern CSS |
| material-ui | mui/material-ui | Pure CSS, config in external package |
| primer-css | primer/css | GitHub's design system, custom plugins |
| carbon | carbon-design-system/carbon | IBM design system, very strict config |

### Key metrics

- **Parity score** = matching / (matching + FN + FP) * 100
- **FN (False Negatives)** = Stylelint reports but Gale misses
- **FP (False Positives)** = Gale reports but Stylelint does not

---

## CLI flags

| Flag | Description |
|------|-------------|
| `<files>` | Files or directories to lint |
| `--config <path>` | Config file path (auto-detected if omitted) |
| `--formatter <type>` | Output format: `text` (default), `json`, `compact` |
| `--fix` | Auto-fix problems |
| `--quiet` | Only report errors (suppress warnings) |
| `--max-warnings <n>` | Exit with error if warnings exceed threshold |
| `--stdin` | Read source from stdin |
| `--stdin-filename <name>` | Virtual filename for stdin (default: `stdin.css`) |
| `--ignore-path <file>` | Custom ignore file (gitignore syntax) |
| `--no-ignore` | Disable all ignore file processing |
| `--cache` | Enable file caching to skip unchanged files |
| `--cache-location <path>` | Custom cache file path (default: `.gale_cache`) |
| `--init` | Generate starter `gale.json` |
| `--print-config <file>` | Print resolved config for a file as JSON |
| `--lsp` | Start LSP server for editor integration |

---

## Implemented rules (50+)

| Rule | Category |
|------|----------|
| `alpha-value-notation` | Value |
| `annotation-no-unknown` | Annotation |
| `at-rule-no-unknown` | At-rule |
| `at-rule-no-vendor-prefix` | At-rule |
| `block-no-empty` | Block |
| `color-hex-case` | Color |
| `color-hex-length` | Color |
| `color-named` | Color |
| `color-no-invalid-hex` | Color |
| `comment-empty-line-before` | Comment |
| `comment-no-empty` | Comment |
| `custom-property-no-missing-var-function` | Custom property |
| `custom-property-pattern` | Custom property |
| `declaration-block-no-duplicate-custom-properties` | Declaration block |
| `declaration-block-no-duplicate-properties` | Declaration block |
| `declaration-block-no-redundant-longhand-properties` | Declaration block |
| `declaration-block-no-shorthand-property-overrides` | Declaration block |
| `declaration-empty-line-before` | Declaration |
| `declaration-no-important` | Declaration |
| `font-family-no-duplicate-names` | Font family |
| `font-family-no-missing-generic-family-keyword` | Font family |
| `function-calc-no-unspaced-operator` | Function |
| `function-name-case` | Function |
| `function-url-quotes` | Function |
| `import-notation` | Import |
| `keyframe-block-no-duplicate-selectors` | Keyframe |
| `keyframe-declaration-no-important` | Keyframe |
| `length-zero-no-unit` | Length |
| `max-nesting-depth` | Nesting |
| `media-feature-name-no-unknown` | Media |
| `media-query-no-invalid` | Media |
| `no-descending-specificity` | Specificity |
| `no-duplicate-at-import-rules` | Import |
| `no-duplicate-selectors` | Selector |
| `no-empty-source` | Source |
| `no-invalid-double-slash-comments` | Comment |
| `no-invalid-position-at-import-rule` | Import |
| `no-invalid-position-declaration` | Declaration |
| `no-irregular-whitespace` | Whitespace |
| `no-unknown-animations` | Animation |
| `number-max-precision` | Number |
| `property-no-unknown` | Property |
| `property-no-vendor-prefix` | Property |
| `rule-empty-line-before` | Rule |
| `selector-class-pattern` | Selector |
| `selector-max-compound-selectors` | Selector |
| `selector-max-id` | Selector |
| `selector-no-qualifying-type` | Selector |
| `selector-pseudo-class-no-unknown` | Selector |
| `selector-pseudo-element-colon-notation` | Selector |
| `selector-pseudo-element-no-unknown` | Selector |
| `selector-type-no-unknown` | Selector |
| `shorthand-property-no-redundant-values` | Shorthand |
| `string-no-newline` | String |
| `unit-no-unknown` | Unit |
| `value-keyword-case` | Value |
| `value-no-vendor-prefix` | Value |

---

## Current status and known gaps

### What works

- Full CSS parsing via lightningcss (with nesting support and error recovery)
- SCSS and Less parsing via raffia (basic support)
- 50+ core Stylelint rules implemented
- Config loading from all Stylelint config formats
- `extends` with built-in presets (`gale:recommended`, `gale:all`)
- `extends` with npm packages (reads from `node_modules/`)
- `extends` with relative file paths
- Inline disable comments (`gale-*` and `stylelint-*` prefixes)
- Auto-fix for rules that support it
- JSON/text/compact output formatters matching Stylelint format
- File caching
- LSP server
- Parallel linting via rayon

### What needs work

1. **SCSS/Less rule accuracy** -- SCSS-specific syntax (`$variables`, `@include`, `@if`, `#{}` interpolation) triggers false positives in rules like `at-rule-no-unknown` and `comment-no-empty`
2. **Plugin system** -- No support for custom/third-party rules (Stylelint plugins). This is a major gap for repos using `@stylistic/*`, `scss/*`, etc.
3. **`cosmiconfig`-style resolution** -- Stylelint uses `cosmiconfig` which also checks `package.json` `stylelint` field; Gale does not
4. **Sass (indented syntax)** -- Returns `UnsupportedSyntax` error
5. **Rule options parity** -- Some rules accept options in Stylelint that Gale does not yet handle (e.g., `ignoreAtRules` for `at-rule-no-unknown`)
6. **npm distribution** -- Set up following Biome's `optionalDependencies` pattern (`npm/gale-linter/` + `npm/@gale-linter/cli-*`). See `PUBLISHING.md` for details. Not yet published.

---

## Key decisions and constraints

### Parser: lightningcss + raffia

- **lightningcss** is used for CSS parsing. It is extremely fast and production-grade. It uses `ParserFlags::NESTING` for CSS nesting support and `error_recovery: true` to continue parsing after errors.
- **raffia** is used for SCSS and Less parsing. It provides a different AST that is converted to the same `CssNode` representation.
- The parser produces an **owned, lifetime-free AST** (`CssNode` enum) so nodes can be stored, cloned, and serialized freely.
- `Span` uses **byte offsets** (not line/column). Line/column conversion is done via `SourceLineIndex` which uses binary search for O(log n) lookups.

### Config compatibility

- Gale reads Stylelint config files directly -- no migration needed
- Rule names are identical to Stylelint's
- Rule config format (`true`/`"error"`/`["error", options]`) is identical
- `extends` supports the same patterns (built-in presets, npm packages, relative paths)
- `ignorePatterns` field is supported

### Output compatibility

- JSON output format matches Stylelint's exactly (array of `{source, warnings}` objects)
- Text output format mimics Stylelint's string formatter
- Exit codes match: 0 for clean, 1 for errors

### Performance

- Files are linted in parallel using `rayon::par_iter()`
- File discovery uses the `ignore` crate (same as ripgrep) for fast directory walking
- `SourceLineIndex` provides O(log n) offset-to-line conversion via binary search
- Optional file caching skips unchanged clean files
- `GALE_DEBUG_PERF=1` emits per-phase timing to stderr

---

## Code conventions

- **Rule struct names** are PascalCase versions of the kebab-case rule name (e.g., `block-no-empty` -> `BlockNoEmpty`)
- **Rule files** use snake_case matching the struct name (e.g., `block_no_empty.rs`)
- Every rule file includes `#[cfg(test)] mod tests` with unit tests
- The `data.rs` module in `gale_linter` contains sorted arrays of known CSS identifiers (properties, at-rules, pseudo-classes, pseudo-elements, units) for validation rules, using case-insensitive binary search
- Diagnostics use the builder pattern: `Diagnostic::new(name, msg).severity(s).span(sp)`
- Spans always use **byte offsets** from the start of the source
- The codebase uses Rust 2024 edition
- Tracing is controlled via `GALE_LOG` env var (e.g., `GALE_LOG=debug`)
- File ignore supports `.galeignore` files (gitignore syntax) in addition to `.gitignore`

---

## Project structure

```
gale/
  Cargo.toml              Workspace root
  src/main.rs             Binary entrypoint (delegates to gale_cli::run)
  crates/
    gale_cli/             CLI, file discovery, orchestration, caching
    gale_config/          Config loading, parsing, extends resolution, presets
    gale_css_parser/      lightningcss + raffia wrapper, CssNode AST
    gale_diagnostics/     Span, Diagnostic, LintResult, Fix/Edit, apply_fixes
    gale_formatter/       Text/JSON/Compact output formatters
    gale_linter/          Rule trait, registry, runner, built-in rules
      src/
        rule.rs           Rule trait + RuleContext
        registry.rs       RuleRegistry
        runner.rs         LintRunner + inline disable comment processing
        data.rs           Known CSS identifiers (properties, units, etc.)
        rules/
          mod.rs          Module declarations + register_all()
          block_no_empty.rs
          color_no_invalid_hex.rs
          ... (50+ rule files)
    gale_lsp/             LSP server
  benchmarks/
    run-benchmark.sh      Benchmark vs Stylelint (uses hyperfine)
    generate-benchmark.sh Generate test fixtures
  tests/
    differential/
      run.py              Differential testing harness
      repos.json          Test corpus definition
      README.md           Documentation
```
