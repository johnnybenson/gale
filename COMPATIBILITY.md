# Compatibility Matrix

Last updated: 2026-03-30

Gale is tested weekly against popular open-source repositories that use Stylelint.
Both tools run on the same files with the same config. Results are compared automatically.

| Repository | Stars | Files | Pass | FP | FN | Speedup |
|------------|-------|-------|------|----|----|---------|
| [twbs/bootstrap](https://github.com/twbs/bootstrap) | 168K | 97/98 | 99% | 1 | 0 | 25.8x |
| [grafana/grafana](https://github.com/grafana/grafana) | 62K | 0/0 | N/A | 0 | 0 | ?x |
| [wordpress/gutenberg](https://github.com/wordpress/gutenberg) | 10K | 694/694 | 100% | 0 | 0 | 35.5x |
| [primer/css](https://github.com/primer/css) | 12K | 113/113 | 100% | 0 | 0 | 2.2x |

### Legend

- **Files**: Matching files / Total files analyzed
- **Pass**: Percentage of files where Gale and Stylelint produce identical output
- **FP**: False positives — warnings Gale reports but Stylelint does not
- **FN**: False negatives — warnings Stylelint reports but Gale misses
- **Speedup**: How many times faster Gale is compared to Stylelint

### How to reproduce

```bash
git clone https://github.com/user/gale && cd gale
cargo build --release
python3 tests/differential/run.py bootstrap --benchmark
```

See [benchmarks/](benchmarks/) for the full benchmark script.
