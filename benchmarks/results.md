# Benchmark Results

> Generated on 2026-03-21 (hyperfine, 3+ runs + warmup per repo)
> System: Darwin arm64 (Apple M4 Max) | macOS 25.3.0
> Node: v22.21.1 | Rust: 1.90.0

## Performance

| Repository | Files | Stylelint | Gale | Speedup |
|------------|------:|----------:|-----:|--------:|
| bootstrap | 98 | ~1.7s | ~12ms | 147x |
| carbon | 1004 | ~10s | ~112ms | 89x |
| patternfly | 204 | ~6.0s | ~86ms | 70x |
| primer-css | 113 | ~1.4s | ~21ms | 68x |
| gutenberg | 746 | ~6.3s | ~157ms | 40x |
| govuk-frontend | 163 | ~1.7s | ~43ms | 40x |
| freecodecamp | 88 | ~0.6s | ~33ms | 18x |
| material-ui | 43 | ~0.5s | ~52ms | 10x |
| grafana | 12 | ~0.4s | ~96ms | 5x |

## Parity (Correctness)

Verified via differential testing (`tests/differential/run.py`):

| Repository | Files | False Positives | False Negatives |
|------------|------:|----------------:|----------------:|
| bootstrap | 98 | 0 | 0 |
| carbon | 1004 | 0 | 0 |
| freecodecamp | 88 | 0 | 0 |
| grafana | 12 | 0 | 0 |
| govuk-frontend | 163 | 0 | 0 |
| gutenberg | 746 | 0 | 0 |
| material-ui | 43 | 0 | 0 |
| patternfly | 204 | 0 | 0 |
| primer-css | 113 | 0 | 0 |

---

*False Positives = Gale reports but Stylelint does not. False Negatives = Stylelint reports but Gale misses.*
*Only rules implemented in Gale are compared. Plugin-only rules are excluded.*

Reproduce these results: `./benchmarks/benchmark.sh`
