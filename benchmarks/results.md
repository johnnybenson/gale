# Benchmark Results

> **These results need to be regenerated.** Run `./benchmarks/benchmark.sh` to produce fresh numbers.
>
> The previous values in this file were not generated from an actual benchmark run and have been removed.

## Performance

| Repository | Files | Stylelint | Gale | Speedup |
|------------|------:|----------:|-----:|--------:|
| *(run `./benchmarks/benchmark.sh` to populate)* | | | | |

## Parity (Correctness)

Verified via differential testing (`tests/differential/run.py`):

| Repository | Files | False Positives | False Negatives |
|------------|------:|----------------:|----------------:|
| *(run `python tests/differential/run.py` to populate)* | | | |

---

*False Positives = Gale reports but Stylelint does not. False Negatives = Stylelint reports but Gale misses.*
*Only rules implemented in Gale are compared. Plugin-only rules are excluded.*

Reproduce these results: `./benchmarks/benchmark.sh`
