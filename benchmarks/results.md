# Benchmark Results

> Generated on 2026-03-22 14:49 UTC
> System: Darwin arm64 | 25.3.0
> Node: v24.14.0 | Rust: 1.90.0

## Performance

| Repository | Files | Stylelint | Gale | Speedup |
|------------|------:|----------:|-----:|--------:|
| grafana | 12 | 0.401s | 0.070s | 5.7x |
| material-ui | 39 | 0.397s | 0.049s | 8.1x |
| freecodecamp | 88 | 0.543s | 0.025s | 21.7x |
| govuk-frontend | 163 | 1.614s | 0.011s | 146.7x |
| spectrum-css | 236 | 2.952s | 0.011s | 268.4x |
| angular-components | 620 | 1.843s | 0.021s | 87.8x |
| docusaurus | 213 | 0.496s | 0.184s | 2.7x |
| discourse | 355 | 1.438s | 0.021s | 68.5x |
| wp-calypso | 1938 | 13.223s | 0.131s | 100.9x |
| mattermost | 0 | SKIP | SKIP | SKIP |

## Parity (Correctness)

| Repository | Files Tested | False Positives | False Negatives |
|------------|-------------:|----------------:|----------------:|
| grafana | 0 | 0 | 0 |
| material-ui | 0 | 0 | 0 |
| freecodecamp | 0 | 0 | 0 |
| govuk-frontend | 0 | 0 | 0 |
| spectrum-css | 0 | 0 | 0 |
| angular-components | 0 | 0 | 0 |
| docusaurus | 0 | 0 | 0 |
| discourse | 0 | 0 | 0 |
| wp-calypso | 0 | 0 | 0 |
| mattermost | SKIP | SKIP | SKIP |

---

*False Positives = Gale reports but Stylelint does not. False Negatives = Stylelint reports but Gale misses.*
*Only rules implemented in Gale are compared. Plugin-only rules are excluded.*

Reproduce these results: `./benchmarks/benchmark.sh`
