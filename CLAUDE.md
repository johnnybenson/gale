# Gale — An extremely fast CSS linter, written in Rust

## Project structure

```
crates/
  gale_diagnostics/  — Span, Diagnostic, LintResult, Fix types
  gale_css_parser/   — CSS parser wrapper (lightningcss for CSS, raffia for SCSS/Less planned)
  gale_config/       — Config loading (.stylelintrc compatible + gale.json/toml native)
  gale_linter/       — Rule trait, registry, runner, built-in rules
  gale_formatter/    — Output formatters (text, json, compact)
  gale_cli/          — CLI entry point (clap)
src/main.rs          — Binary entrypoint (delegates to gale_cli::run)
```

## Build & Test

```sh
cargo build --release
cargo test --workspace
```

## Adding a new rule

1. Create `crates/gale_linter/src/rules/your_rule_name.rs`
2. Implement the `Rule` trait
3. Add `pub mod your_rule_name;` to `crates/gale_linter/src/rules/mod.rs`
4. Register it in `register_all()`

## Benchmark

```sh
bash benchmarks/run-benchmark.sh
```

## Key decisions
- Parser: lightningcss (CSS) + raffia planned (SCSS/Less)
- lightningcss `Span` stores line/column, not byte offsets
- Config: compatible with Stylelint (.stylelintrc.json, etc.) + native gale.json/toml
- npm distribution: follow Biome's optionalDependencies pattern (not yet set up)
