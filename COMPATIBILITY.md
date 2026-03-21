# Compatibility Matrix

Last updated: 2026-03-21

Gale is tested against popular open-source repositories that use Stylelint.
Both tools run on the same files with the same config. Results are compared automatically.

| Repository | Stars | Files | Pass | FP | FN | Speedup |
|------------|-------|-------|------|----|----|---------|
| [twbs/bootstrap](https://github.com/twbs/bootstrap) | 172K | 98/98 | 100% | 0 | 0 | 147x |
| [carbon-design-system/carbon](https://github.com/carbon-design-system/carbon) | 8K | 1004/1004 | 100% | 0 | 0 | 89x |
| [patternfly/patternfly](https://github.com/patternfly/patternfly) | 700 | 204/204 | 100% | 0 | 0 | 70x |
| [primer/css](https://github.com/primer/css) | 12K | 113/113 | 100% | 0 | 0 | 68x |
| [wordpress/gutenberg](https://github.com/wordpress/gutenberg) | 10K | 746/746 | 100% | 0 | 0 | 40x |
| [alphagov/govuk-frontend](https://github.com/alphagov/govuk-frontend) | 4.8K | 163/163 | 100% | 0 | 0 | 40x |
| [freeCodeCamp/freeCodeCamp](https://github.com/freeCodeCamp/freeCodeCamp) | 439K | 88/88 | 100% | 0 | 0 | 18x |
| [mui/material-ui](https://github.com/mui/material-ui) | 95K | 43/43 | 100% | 0 | 0 | 10x |
| [grafana/grafana](https://github.com/grafana/grafana) | 66K | 12/12 | 100% | 0 | 0 | 5x |

### Legend

- **Files**: Matching files / Total files analyzed
- **Pass**: Percentage of files where Gale and Stylelint produce identical output
- **FP**: False positives — warnings Gale reports but Stylelint does not
- **FN**: False negatives — warnings Stylelint reports but Gale misses
- **Speedup**: How many times faster Gale is compared to Stylelint (hyperfine, 10 runs)

### Notes

- **9/9 repos at 100% parity**: Zero false positives, zero false negatives across all tested repositories.
- **Speedup range**: 5x to 147x faster than Stylelint. Larger repos with more files benefit most from parallel linting.
- Benchmarks measured with [hyperfine](https://github.com/sharkdp/hyperfine) (10 runs, 3 warmup) on Apple M4 Max.

### How to reproduce

```bash
git clone https://github.com/user/gale && cd gale
cargo build --release
python3 tests/differential/run.py --benchmark         # all repos
python3 tests/differential/run.py bootstrap --benchmark  # specific repo
bash benchmarks/benchmark.sh                           # hyperfine benchmarks
```

See [benchmarks/](benchmarks/) for the full benchmark script and results.
