# Compatibility Matrix

Last updated: 2026-03-21

Gale is tested against popular open-source repositories that use Stylelint.
Both tools run on the same files with the same config. Results are compared automatically.

| Repository | Stars | Files | Pass | FP | FN | Speedup |
|------------|-------|-------|------|----|----|---------|
| [twbs/bootstrap](https://github.com/twbs/bootstrap) | 172K | 98/98 | 100% | 0 | 0 | 67x |
| [wordpress/gutenberg](https://github.com/wordpress/gutenberg) | 10K | 746/746 | 100% | 0 | 0 | 60x |
| [freeCodeCamp/freeCodeCamp](https://github.com/freeCodeCamp/freeCodeCamp) | 439K | 88/88 | 100% | 0 | 0 | 62x |
| [patternfly/patternfly](https://github.com/patternfly/patternfly) | 700 | 204/204 | 100% | 0 | 0 | 66x |
| [alphagov/govuk-frontend](https://github.com/alphagov/govuk-frontend) | 4.8K | 163/163 | 100% | 0 | 0 | 54x |
| [primer/css](https://github.com/primer/css) | 12K | 113/113 | 100% | 0 | 0 | 90x |
| [mui/material-ui](https://github.com/mui/material-ui) | 95K | 43/43 | 100% | 0 | 0 | 105x |
| [carbon-design-system/carbon](https://github.com/carbon-design-system/carbon) | 8K | 1002/1004 | 99.8% | 13 | 0 | 95x |
| [grafana/grafana](https://github.com/grafana/grafana) | 66K | 11/12 | 91.7% | 3 | 3 | 52x |

### Legend

- **Files**: Matching files / Total files analyzed
- **Pass**: Percentage of files where Gale and Stylelint produce identical output
- **FP**: False positives — warnings Gale reports but Stylelint does not
- **FN**: False negatives — warnings Stylelint reports but Gale misses
- **Speedup**: How many times faster Gale is compared to Stylelint

### Notes

- **7 repos at 100% parity**: Bootstrap, Gutenberg, freeCodeCamp, PatternFly, GOV.UK Frontend, Primer CSS, and Material UI all have zero differences.
- **Carbon**: 13 FP from `max-nesting-depth` in 2 web-component story files. Gale does not yet handle Carbon's `ignoreAtRules` option for nesting depth. No logic bugs in core rules.
- **Grafana**: 3 FP/3 FN from minor message text and line offset differences in `declaration-block-no-duplicate-properties` on a single vendor CSS file. Same warnings are detected — just formatted slightly differently.
- **Speedup range**: 52x to 105x faster than Stylelint across all tested repos.

### How to reproduce

```bash
git clone https://github.com/user/gale && cd gale
cargo build --release
python3 tests/differential/run.py --benchmark         # all repos
python3 tests/differential/run.py bootstrap --benchmark  # specific repo
```

See [benchmarks/](benchmarks/) for the full benchmark script.
