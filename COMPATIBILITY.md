# Compatibility Matrix

Last updated: 2026-03-21

Gale is tested against popular open-source repositories that use Stylelint.
Both tools run on the same files with the same config. Results are compared automatically.

| Repository | Stars | Files | Pass | FP | FN | Speedup |
|------------|-------|-------|------|----|----|---------|
| [twbs/bootstrap](https://github.com/twbs/bootstrap) | 168K | 99/99 | 100% | 0 | 0 | 4.9x |
| [wordpress/gutenberg](https://github.com/wordpress/gutenberg) | 10K | 762/775 | 98.3% | 13 | 0 | 21-85x |

### Legend

- **Files**: Matching files / Total files analyzed
- **Pass**: Percentage of files where Gale and Stylelint produce identical output
- **FP**: False positives — warnings Gale reports but Stylelint does not
- **FN**: False negatives — warnings Stylelint reports but Gale misses
- **Speedup**: How many times faster Gale is compared to Stylelint

### Notes

- Bootstrap: Perfect parity. Zero differences across 99 SCSS files.
- Gutenberg: 13 remaining differences are from `.stylelintignore` file handling (5) and minor message text differences (7+1). No logic bugs.

### How to reproduce

```bash
git clone https://github.com/user/gale && cd gale
cargo build --release
python3 tests/differential/run.py bootstrap --benchmark
python3 tests/differential/run.py gutenberg --benchmark
```

See [benchmarks/](benchmarks/) for the full benchmark script.
