#!/usr/bin/env python3
"""
Generate COMPATIBILITY.md from differential test results.

Usage: python3 scripts/generate-compatibility-matrix.py results/
"""

import re
import sys
from datetime import datetime, timezone
from pathlib import Path

REPO_META = {
    "bootstrap": {"full": "twbs/bootstrap", "stars": "168K", "description": "The most popular CSS framework"},
    "gutenberg": {"full": "wordpress/gutenberg", "stars": "10K", "description": "WordPress block editor"},
    "grafana": {"full": "grafana/grafana", "stars": "62K", "description": "Observability platform"},
    "primer-css": {"full": "primer/css", "stars": "12K", "description": "GitHub's design system"},
    "carbon": {"full": "carbon-design-system/carbon", "stars": "7K", "description": "IBM's design system"},
    "material-ui": {"full": "mui/material-ui", "stars": "94K", "description": "React UI library"},
}


def parse_result(text: str) -> dict:
    """Extract metrics from a differential test result."""
    metrics = {}

    m = re.search(r"Files analyzed:\s+(\d+)", text)
    metrics["files_total"] = int(m.group(1)) if m else 0

    m = re.search(r"Files matching:\s+(\d+)", text)
    metrics["files_match"] = int(m.group(1)) if m else 0

    m = re.search(r"Gale-only \(FP\):\s+(\d+)", text)
    metrics["fp"] = int(m.group(1)) if m else 0

    m = re.search(r"Stylelint-only \(FN\):\s+(\d+)", text)
    metrics["fn"] = int(m.group(1)) if m else 0

    m = re.search(r"Speedup:\s+([\d.]+)x", text)
    metrics["speedup"] = m.group(1) if m else "?"

    m = re.search(r"Gale:\s+([\d.]+)s", text)
    metrics["gale_time"] = m.group(1) if m else "?"

    m = re.search(r"Stylelint:\s+([\d.]+)s", text)
    metrics["stylelint_time"] = m.group(1) if m else "?"

    return metrics


def main():
    results_dir = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("results")

    rows = []
    for result_dir in sorted(results_dir.iterdir()):
        if not result_dir.is_dir():
            continue

        name = result_dir.name.replace("result-", "")
        result_file = result_dir / "result.txt"
        if not result_file.exists():
            continue

        text = result_file.read_text()
        metrics = parse_result(text)
        meta = REPO_META.get(name, {"full": name, "stars": "?", "description": ""})

        total = metrics["files_total"]
        match = metrics["files_match"]
        pct = f"{match/total*100:.0f}%" if total > 0 else "N/A"

        rows.append({
            "name": name,
            "full": meta["full"],
            "stars": meta["stars"],
            "description": meta["description"],
            "files": f"{match}/{total}",
            "pct": pct,
            "fp": metrics["fp"],
            "fn": metrics["fn"],
            "speedup": metrics["speedup"],
            "gale_time": metrics["gale_time"],
            "stylelint_time": metrics["stylelint_time"],
        })

    now = datetime.now(timezone.utc).strftime("%Y-%m-%d")

    print("# Compatibility Matrix")
    print()
    print(f"Last updated: {now}")
    print()
    print("Gale is tested weekly against popular open-source repositories that use Stylelint.")
    print("Both tools run on the same files with the same config. Results are compared automatically.")
    print()
    print("| Repository | Stars | Files | Pass | FP | FN | Speedup |")
    print("|------------|-------|-------|------|----|----|---------|")

    for r in rows:
        repo_link = f"[{r['full']}](https://github.com/{r['full']})"
        print(f"| {repo_link} | {r['stars']} | {r['files']} | {r['pct']} | {r['fp']} | {r['fn']} | {r['speedup']}x |")

    print()
    print("### Legend")
    print()
    print("- **Files**: Matching files / Total files analyzed")
    print("- **Pass**: Percentage of files where Gale and Stylelint produce identical output")
    print("- **FP**: False positives — warnings Gale reports but Stylelint does not")
    print("- **FN**: False negatives — warnings Stylelint reports but Gale misses")
    print("- **Speedup**: How many times faster Gale is compared to Stylelint")
    print()
    print("### How to reproduce")
    print()
    print("```bash")
    print("git clone https://github.com/user/gale && cd gale")
    print("cargo build --release")
    print("python3 tests/differential/run.py bootstrap --benchmark")
    print("```")
    print()
    print("See [benchmarks/](benchmarks/) for the full benchmark script.")


if __name__ == "__main__":
    main()
