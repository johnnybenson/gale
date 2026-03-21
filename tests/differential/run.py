#!/usr/bin/env python3
"""
Differential testing harness for Gale vs Stylelint.

Clones public repos, runs both linters with JSON output (using the repo's own
Stylelint config), and compares results. This tests whether Gale is truly a
drop-in replacement.

Usage:
    python run.py                    # Run all repos
    python run.py grafana bootstrap  # Run specific repos
    python run.py --list             # List available repos
    python run.py --update           # Re-clone repos (force fresh)
    python run.py --css-only         # Only test .css files (skip SCSS/Less)
    python run.py --benchmark        # Also measure and report execution times
"""

import argparse
import json
import subprocess
import sys
import time
from collections import Counter
from pathlib import Path

# ---------------------------------------------------------------------------
# Paths
# ---------------------------------------------------------------------------

SCRIPT_DIR = Path(__file__).parent
REPOS_JSON = SCRIPT_DIR / "repos.json"
CLONES_DIR = SCRIPT_DIR / ".clones"
RESULTS_DIR = SCRIPT_DIR / "results"
GALE_ROOT = SCRIPT_DIR.parent.parent  # gale/

# Known Gale-supported rules (core Stylelint rules implemented in Gale).
# Used to filter Stylelint output so we only compare what Gale can check.
GALE_RULES = {
    "alpha-value-notation", "annotation-no-unknown", "at-rule-no-unknown",
    "at-rule-no-vendor-prefix", "block-no-empty", "color-hex-case",
    "color-hex-length", "color-named", "color-no-invalid-hex",
    "comment-empty-line-before", "comment-no-empty",
    "custom-property-no-missing-var-function", "custom-property-pattern",
    "declaration-block-no-duplicate-custom-properties",
    "declaration-block-no-duplicate-properties",
    "declaration-block-no-redundant-longhand-properties",
    "declaration-block-no-shorthand-property-overrides",
    "declaration-empty-line-before", "declaration-no-important",
    "font-family-no-duplicate-names",
    "font-family-no-missing-generic-family-keyword",
    "function-calc-no-unspaced-operator", "function-name-case",
    "function-url-quotes", "import-notation",
    "keyframe-block-no-duplicate-selectors",
    "keyframe-declaration-no-important", "length-zero-no-unit",
    "max-nesting-depth", "media-feature-name-no-unknown",
    "media-query-no-invalid", "no-descending-specificity",
    "no-duplicate-at-import-rules", "no-duplicate-selectors",
    "no-empty-source", "no-invalid-double-slash-comments",
    "no-invalid-position-at-import-rule", "no-invalid-position-declaration",
    "no-irregular-whitespace", "no-unknown-animations",
    "number-max-precision", "property-no-unknown", "property-no-vendor-prefix",
    "rule-empty-line-before", "selector-class-pattern",
    "selector-max-compound-selectors", "selector-max-id",
    "selector-no-qualifying-type", "selector-pseudo-class-no-unknown",
    "selector-pseudo-element-colon-notation",
    "selector-pseudo-element-no-unknown", "selector-type-no-unknown",
    "shorthand-property-no-redundant-values", "string-no-newline",
    "unit-no-unknown", "value-keyword-case", "value-no-vendor-prefix",
}

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def load_repos() -> list[dict]:
    with open(REPOS_JSON) as f:
        return json.load(f)


def run_cmd(cmd: list[str], cwd: str | None = None, timeout: int = 300) -> subprocess.CompletedProcess:
    """Run a command and return the result. Does not raise on non-zero exit."""
    return subprocess.run(
        cmd, cwd=cwd, capture_output=True, text=True, timeout=timeout,
    )


def clone_repo(repo: str, branch: str, dest: Path, force: bool = False) -> bool:
    """Shallow-clone a repo. Returns True if clone dir exists after."""
    if dest.exists() and not force:
        print(f"  [skip] Already cloned: {dest.name}")
        return True

    if dest.exists():
        import shutil
        shutil.rmtree(dest)

    print(f"  [clone] {repo} @ {branch} -> {dest.name}")
    result = run_cmd(
        ["git", "clone", "--depth", "1", "--branch", branch,
         f"https://github.com/{repo}.git", str(dest)],
        timeout=120,
    )
    if result.returncode != 0:
        print(f"  [error] Clone failed: {result.stderr.strip()}")
        return False
    return True


def detect_package_manager(clone_dir: Path) -> str:
    if (clone_dir / "pnpm-lock.yaml").exists():
        return "pnpm"
    if (clone_dir / "yarn.lock").exists():
        return "yarn"
    if (clone_dir / "bun.lockb").exists() or (clone_dir / "bun.lock").exists():
        return "bun"
    return "npm"


def install_deps(clone_dir: Path) -> bool:
    """Install npm dependencies (needed for Stylelint and its plugins)."""
    node_modules = clone_dir / "node_modules"
    if node_modules.exists():
        print(f"  [skip] node_modules already exists")
        return True

    if not (clone_dir / "package.json").exists():
        print(f"  [warn] No package.json found")
        return False

    pm = detect_package_manager(clone_dir)
    print(f"  [install] Installing dependencies with {pm}...")

    run_cmd(["corepack", "enable"], cwd=str(clone_dir), timeout=30)

    if pm == "pnpm":
        cmd = ["pnpm", "install", "--ignore-scripts", "--no-frozen-lockfile"]
    elif pm == "yarn":
        env_file = clone_dir / ".yarnrc.yml"
        if env_file.exists():
            content = env_file.read_text()
            if "nodeLinker" not in content:
                with open(env_file, "a") as f:
                    f.write("\nnodeLinker: node-modules\n")
        cmd = ["yarn", "install", "--mode", "skip-build"]
    elif pm == "bun":
        cmd = ["bun", "install", "--ignore-scripts"]
    else:
        cmd = ["npm", "install", "--ignore-scripts"]

    result = run_cmd(cmd, cwd=str(clone_dir), timeout=300)
    if result.returncode != 0:
        print(f"  [error] {pm} install failed: {result.stderr.strip()[:200]}")
        return False
    return True


def find_css_files(clone_dir: Path, search_paths: list[str], css_only: bool = False) -> list[str]:
    """Find all CSS/SCSS/Less files under the given paths."""
    extensions = {".css"} if css_only else {".css", ".scss", ".less", ".sass"}
    files = []

    for search_path in search_paths:
        root = clone_dir / search_path
        if not root.exists():
            continue
        for f in root.rglob("*"):
            if (f.suffix in extensions
                    and "node_modules" not in f.parts
                    and ".git" not in f.parts):
                files.append(str(f.relative_to(clone_dir)))

    return sorted(files)


# ---------------------------------------------------------------------------
# Linter runners
# ---------------------------------------------------------------------------


def run_stylelint(clone_dir: Path, files: list[str]) -> list[dict] | None:
    """Run Stylelint on the given files and return parsed JSON output."""
    stylelint_bin = clone_dir / "node_modules" / ".bin" / "stylelint"
    if not stylelint_bin.exists():
        print(f"  [error] Stylelint binary not found")
        return None

    all_results = []
    batch_size = 50

    for i in range(0, len(files), batch_size):
        batch = files[i:i + batch_size]
        cmd = [str(stylelint_bin), "--formatter", "json", "--no-color"] + batch
        result = run_cmd(cmd, cwd=str(clone_dir), timeout=120)

        # Stylelint 16+ outputs JSON to stderr instead of stdout.
        # Try stdout first, then fall back to stderr.
        output = result.stdout.strip()
        if not output:
            output = result.stderr.strip()

        if result.returncode == 2 and not output:
            print(f"  [error] Stylelint config error (exit 2, no output)")
            return None

        if not output:
            continue

        try:
            all_results.extend(json.loads(output))
        except json.JSONDecodeError as e:
            if result.returncode == 2:
                print(f"  [error] Stylelint config error: {output[:200]}")
                return None
            print(f"  [error] Stylelint JSON parse error: {e}")
            return None

    return all_results


def run_gale(clone_dir: Path, files: list[str], gale_bin: Path) -> list[dict] | None:
    """Run Gale on the given files and return parsed JSON output.

    Gale reads the repo's .stylelintrc / gale.json automatically.
    """
    if not gale_bin.exists():
        print(f"  [error] Gale binary not found")
        return None

    all_results = []
    batch_size = 50

    for i in range(0, len(files), batch_size):
        batch = files[i:i + batch_size]
        cmd = [str(gale_bin), "--formatter", "json"] + batch
        result = run_cmd(cmd, cwd=str(clone_dir), timeout=120)

        # Capture stderr for debugging
        if result.stderr.strip():
            for line in result.stderr.strip().split("\n")[:3]:
                print(f"  [gale stderr] {line}")

        stdout = result.stdout.strip()
        if not stdout:
            continue

        try:
            all_results.extend(json.loads(stdout))
        except json.JSONDecodeError as e:
            print(f"  [error] Gale JSON parse error: {e}")
            return None

    return all_results


# ---------------------------------------------------------------------------
# Comparison
# ---------------------------------------------------------------------------


def normalize_results(
    results: list[dict],
    clone_dir: Path,
    filter_rules: set[str] | None = None,
) -> dict[str, list[dict]]:
    """Normalize linter output into a comparable structure.

    Returns: { relative_path: [sorted list of {line, column, rule, severity, text}] }
    """
    normalized = {}

    for entry in results:
        source = entry.get("source", "")
        try:
            rel_path = str(Path(source).relative_to(clone_dir))
        except ValueError:
            rel_path = source

        warnings = []
        for w in entry.get("warnings", []):
            rule = w.get("rule")
            if filter_rules and rule not in filter_rules:
                continue
            # Stylelint appends " (rule-name)" to every message; strip it so
            # comparisons focus on the actual message content.
            text = w.get("text", "")
            if rule and text.endswith(f" ({rule})"):
                text = text[: -(len(rule) + 3)]
            warnings.append({
                "line": w.get("line"),
                "column": w.get("column"),
                "rule": rule,
                "severity": w.get("severity"),
                "text": text,
            })

        warnings.sort(key=lambda w: (w["line"] or 0, w["column"] or 0, w["rule"] or ""))
        normalized[rel_path] = warnings

    return normalized


def compare_results(
    stylelint: dict[str, list[dict]],
    gale: dict[str, list[dict]],
) -> dict:
    """Compare normalized results and return a report."""
    all_files = sorted(set(list(stylelint.keys()) + list(gale.keys())))

    report = {
        "total_files": len(all_files),
        "files_match": 0,
        "files_differ": 0,
        "stylelint_only_warnings": 0,
        "gale_only_warnings": 0,
        "matching_warnings": 0,
        "diffs": [],
    }

    for file_path in all_files:
        s_warnings = stylelint.get(file_path, [])
        g_warnings = gale.get(file_path, [])

        s_set = {_warning_key(w) for w in s_warnings}
        g_set = {_warning_key(w) for w in g_warnings}

        matching = s_set & g_set
        stylelint_only = s_set - g_set
        gale_only = g_set - s_set

        report["matching_warnings"] += len(matching)
        report["stylelint_only_warnings"] += len(stylelint_only)
        report["gale_only_warnings"] += len(gale_only)

        if stylelint_only or gale_only:
            report["files_differ"] += 1
            report["diffs"].append({
                "file": file_path,
                "matching": len(matching),
                "stylelint_only": [_key_to_dict(k) for k in sorted(stylelint_only)],
                "gale_only": [_key_to_dict(k) for k in sorted(gale_only)],
            })
        else:
            report["files_match"] += 1

    return report


def _warning_key(w: dict) -> tuple:
    return (w["line"], w["column"], w["rule"], w["severity"], w["text"])


def _key_to_dict(key: tuple) -> dict:
    return {"line": key[0], "column": key[1], "rule": key[2],
            "severity": key[3], "text": key[4]}


# ---------------------------------------------------------------------------
# Report
# ---------------------------------------------------------------------------


def print_report(repo_name: str, report: dict):
    total = report["matching_warnings"] + report["stylelint_only_warnings"] + report["gale_only_warnings"]

    print(f"\n{'='*70}")
    print(f"  REPORT: {repo_name}")
    print(f"{'='*70}")
    print(f"  Files analyzed:        {report['total_files']}")
    print(f"  Files matching:        {report['files_match']}")
    print(f"  Files with diffs:      {report['files_differ']}")
    print(f"  Matching warnings:     {report['matching_warnings']}")
    print(f"  Stylelint-only (FN):   {report['stylelint_only_warnings']}")
    print(f"  Gale-only (FP):        {report['gale_only_warnings']}")

    if total > 0:
        parity = report["matching_warnings"] / total * 100
        print(f"  Parity score:          {parity:.1f}%")

    if "stylelint_time" in report:
        s_time = report["stylelint_time"]
        g_time = report["gale_time"]
        speedup = s_time / g_time if g_time > 0 else float("inf")
        print(f"\n  Performance:")
        print(f"    Stylelint:   {s_time:.2f}s")
        print(f"    Gale:        {g_time:.2f}s")
        print(f"    Speedup:     {speedup:.1f}x faster")

    # Breakdown by rule
    rule_fn: Counter = Counter()
    rule_fp: Counter = Counter()
    for diff in report["diffs"]:
        for w in diff["stylelint_only"]:
            rule_fn[w["rule"]] += 1
        for w in diff["gale_only"]:
            rule_fp[w["rule"]] += 1

    if rule_fn or rule_fp:
        all_rules = sorted(set(list(rule_fn.keys()) + list(rule_fp.keys())))
        print(f"\n  Rule breakdown:")
        print(f"  {'Rule':<50} {'FN':<8} {'FP':<8}")
        print(f"  {'─'*66}")
        for rule in all_rules:
            print(f"  {rule:<50} {rule_fn[rule]:<8} {rule_fp[rule]:<8}")

    # Show first N file diffs
    max_diffs = 5
    shown = 0
    for diff in report["diffs"]:
        if shown >= max_diffs:
            remaining = len(report["diffs"]) - max_diffs
            print(f"\n  ... and {remaining} more files with differences")
            break

        print(f"\n  --- {diff['file']} ({diff['matching']} match, "
              f"{len(diff['stylelint_only'])} FN, {len(diff['gale_only'])} FP)")

        for w in diff["stylelint_only"][:3]:
            print(f"    [FN] {w['line']}:{w['column']} {w['rule']} ({w['severity']}) {w['text']}")
        for w in diff["gale_only"][:3]:
            print(f"    [FP] {w['line']}:{w['column']} {w['rule']} ({w['severity']}) {w['text']}")

        shown += 1

    print(f"\n{'='*70}\n")


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------


def build_gale() -> Path | None:
    print("[build] Building Gale (release)...")
    result = run_cmd(["cargo", "build", "--release"], cwd=str(GALE_ROOT), timeout=300)
    if result.returncode != 0:
        print(f"[error] Gale build failed: {result.stderr.strip()[:500]}")
        return None

    binary = GALE_ROOT / "target" / "release" / "gale"
    if not binary.exists():
        print(f"[error] Gale binary not found")
        return None

    print(f"[build] Gale binary ready")
    return binary


def process_repo(
    repo_config: dict,
    gale_bin: Path,
    force_clone: bool = False,
    css_only: bool = False,
    benchmark: bool = False,
) -> dict | None:
    """Process a single repo: clone, install, lint with both tools, compare."""
    name = repo_config["name"]
    repo = repo_config["repo"]
    branch = repo_config["branch"]
    search_paths = repo_config["paths"]

    print(f"\n{'─'*70}")
    print(f"Processing: {name} ({repo})")
    print(f"{'─'*70}")

    clone_dir = CLONES_DIR / name

    if not clone_repo(repo, branch, clone_dir, force=force_clone):
        return None

    if not install_deps(clone_dir):
        print(f"  [warn] Continuing without deps (Stylelint may not work)")

    css_files = find_css_files(clone_dir, search_paths, css_only=css_only)
    ext_label = ".css only" if css_only else "CSS/SCSS/Less"
    print(f"  [files] Found {len(css_files)} {ext_label} files")

    if not css_files:
        print(f"  [skip] No files found")
        return None

    # Run Stylelint (using repo's own config)
    print(f"  [lint] Running Stylelint...")
    t0 = time.time()
    stylelint_results = run_stylelint(clone_dir, css_files)
    stylelint_time = time.time() - t0
    if stylelint_results is None:
        print(f"  [warn] Stylelint failed, skipping comparison")
        return None
    s_count = sum(len(r.get("warnings", [])) for r in stylelint_results)
    print(f"  [lint] Stylelint: {s_count} warnings")

    # Run Gale (using repo's own config — should read .stylelintrc automatically)
    print(f"  [lint] Running Gale...")
    t0 = time.time()
    gale_results = run_gale(clone_dir, css_files, gale_bin)
    gale_time = time.time() - t0
    if gale_results is None:
        print(f"  [warn] Gale failed, skipping comparison")
        return None
    g_count = sum(len(r.get("warnings", [])) for r in gale_results)
    print(f"  [lint] Gale: {g_count} warnings")

    # Normalize & compare
    # Filter Stylelint results to only rules Gale supports (ignore plugin rules)
    s_norm = normalize_results(stylelint_results, clone_dir, filter_rules=GALE_RULES)
    g_norm = normalize_results(gale_results, clone_dir, filter_rules=GALE_RULES)
    report = compare_results(s_norm, g_norm)

    # Save raw results
    RESULTS_DIR.mkdir(parents=True, exist_ok=True)
    with open(RESULTS_DIR / f"{name}_stylelint.json", "w") as f:
        json.dump(stylelint_results, f, indent=2)
    with open(RESULTS_DIR / f"{name}_gale.json", "w") as f:
        json.dump(gale_results, f, indent=2)
    with open(RESULTS_DIR / f"{name}_report.json", "w") as f:
        json.dump(report, f, indent=2)

    if benchmark:
        report["stylelint_time"] = stylelint_time
        report["gale_time"] = gale_time

    print_report(name, report)
    return report


def main():
    parser = argparse.ArgumentParser(description="Differential testing: Gale vs Stylelint")
    parser.add_argument("repos", nargs="*", help="Specific repos to test (by name)")
    parser.add_argument("--list", action="store_true", help="List available repos")
    parser.add_argument("--update", action="store_true", help="Force re-clone repos")
    parser.add_argument("--skip-build", action="store_true", help="Skip building Gale")
    parser.add_argument("--gale-bin", type=str, help="Path to pre-built Gale binary")
    parser.add_argument("--css-only", action="store_true",
                        help="Only test .css files (skip SCSS/Less)")
    parser.add_argument("--benchmark", action="store_true",
                        help="Measure and report execution time of both linters")
    args = parser.parse_args()

    repos = load_repos()

    if args.list:
        print("Available repos:")
        for r in repos:
            print(f"  {r['name']:<20} {r['repo']:<40} {r.get('notes', '')}")
        return

    if args.repos:
        repos = [r for r in repos if r["name"] in args.repos]
        if not repos:
            print(f"No repos matched: {args.repos}")
            sys.exit(1)

    if args.gale_bin:
        gale_bin = Path(args.gale_bin)
        if not gale_bin.exists():
            print(f"[error] Gale binary not found: {gale_bin}")
            sys.exit(1)
    elif args.skip_build:
        gale_bin = GALE_ROOT / "target" / "release" / "gale"
        if not gale_bin.exists():
            print("[error] No Gale binary found. Build first or use --gale-bin")
            sys.exit(1)
    else:
        gale_bin = build_gale()
        if gale_bin is None:
            sys.exit(1)

    CLONES_DIR.mkdir(parents=True, exist_ok=True)
    RESULTS_DIR.mkdir(parents=True, exist_ok=True)

    summaries = []
    for repo_config in repos:
        report = process_repo(
            repo_config, gale_bin,
            force_clone=args.update,
            css_only=args.css_only,
            benchmark=args.benchmark,
        )
        if report:
            summaries.append((repo_config["name"], report))

    if summaries:
        has_benchmark = "stylelint_time" in summaries[0][1]
        print(f"\n{'='*70}")
        print(f"  FINAL SUMMARY")
        print(f"{'='*70}")
        header = f"  {'Repo':<20} {'Files':<8} {'Match':<8} {'FN':<8} {'FP':<8} {'Parity':<8}"
        sep_len = 60
        if has_benchmark:
            header += f"{'Speedup':<10}"
            sep_len = 70
        print(header)
        print(f"  {'─'*sep_len}")
        for name, report in summaries:
            total = report["matching_warnings"] + report["stylelint_only_warnings"] + report["gale_only_warnings"]
            parity = f"{report['matching_warnings'] / total * 100:.1f}%" if total > 0 else "N/A"
            line = (f"  {name:<20} {report['total_files']:<8} "
                    f"{report['matching_warnings']:<8} "
                    f"{report['stylelint_only_warnings']:<8} "
                    f"{report['gale_only_warnings']:<8} "
                    f"{parity:<8}")
            if has_benchmark:
                s_time = report["stylelint_time"]
                g_time = report["gale_time"]
                speedup = s_time / g_time if g_time > 0 else float("inf")
                line += f"{speedup:.1f}x"
            print(line)
        print()


if __name__ == "__main__":
    main()
