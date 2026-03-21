# Benchmark Results

> Generated on 2026-03-21 (hyperfine, 10 runs + 3 warmup per repo)
> System: Darwin arm64 (Apple M4 Max) | macOS 25.3.0
> Node: v22.21.1 | Rust: 1.90.0

## Performance

| Repository | Files | Stylelint | Gale | Speedup |
|------------|------:|----------:|-----:|--------:|
| bootstrap | 98 | 1.665s | 0.029s | 58.1x |
| carbon | 1004 | 10.230s | 0.575s | 17.8x |
| primer-css | 113 | 1.441s | 0.083s | 17.4x |
| patternfly | 204 | 6.002s | 0.823s | 7.3x |
| gutenberg | 746 | 6.319s | 1.110s | 5.7x |
| freecodecamp | 88 | 0.598s | 0.109s | 5.5x |
| govuk-frontend | 163 | 1.718s | 0.443s | 3.9x |
| material-ui | 43 | 0.479s | 0.161s | 3.0x |
| grafana | 12 | 0.433s | 0.197s | 2.2x |

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
