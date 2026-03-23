# Benchmark Results

> Generated on 2026-03-22 17:08 UTC
> System: Darwin arm64 | 25.3.0
> Node: v22.21.1 | Rust: 1.90.0

## Performance

| Repository | Files | Stylelint | Gale | Speedup |
|------------|------:|----------:|-----:|--------:|
| carbon | 1116 | 9.071s | 0.096s | 94.5x |

## Parity (Correctness)

| Repository | Files Tested | False Positives | False Negatives |
|------------|-------------:|----------------:|----------------:|
| carbon | 141 | 1382 | 0 |

---

*False Positives = Gale reports but Stylelint does not. False Negatives = Stylelint reports but Gale misses.*
*Only rules implemented in Gale are compared. Plugin-only rules are excluded.*

Reproduce these results: `./benchmarks/benchmark.sh`
