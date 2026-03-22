# Benchmark Results

> Generated on 2026-03-22 14:42 UTC
> System: Darwin arm64 | 25.3.0
> Node: v24.14.0 | Rust: 1.90.0

## Performance

| Repository | Files | Stylelint | Gale | Speedup |
|------------|------:|----------:|-----:|--------:|
| bootstrap | 99 | 1.569s | 0.011s | 142.6x |
| carbon | 1116 | 8.767s | 0.022s | 398.5x |
| primer-css | 113 | 1.285s | 0.010s | 128.5x |
| patternfly | 204 | 5.298s | 0.015s | 353.2x |
| gutenberg | 775 | 4.715s | 0.042s | 112.3x |

## Parity (Correctness)

| Repository | Files Tested | False Positives | False Negatives |
|------------|-------------:|----------------:|----------------:|
| bootstrap | 0 | 0 | 0 |
| carbon | 0 | 0 | 0 |
| primer-css | 0 | 0 | 0 |
| patternfly | 0 | 0 | 0 |
| gutenberg | 0 | 0 | 0 |

---

*False Positives = Gale reports but Stylelint does not. False Negatives = Stylelint reports but Gale misses.*
*Only rules implemented in Gale are compared. Plugin-only rules are excluded.*

Reproduce these results: `./benchmarks/benchmark.sh`
