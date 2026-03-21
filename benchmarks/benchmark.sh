#!/usr/bin/env bash
# =============================================================================
# Gale vs Stylelint -- Reproducible Benchmark
# =============================================================================
#
# Run this script to independently verify Gale's performance claims.
# It clones real-world CSS repositories, runs both linters via hyperfine,
# and produces a markdown results table.
#
# Usage:
#   ./benchmarks/benchmark.sh              # Full benchmark (Bootstrap + Gutenberg)
#   ./benchmarks/benchmark.sh bootstrap    # Single repo
#   ./benchmarks/benchmark.sh --help       # Show help
#
# Requirements: cargo, node (>=18), hyperfine
# =============================================================================

set -euo pipefail

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$SCRIPT_DIR/.."
CLONES_DIR="$SCRIPT_DIR/.repos"
RESULTS_FILE="$SCRIPT_DIR/results.md"
GALE_BIN="$PROJECT_DIR/target/release/gale"

WARMUP=3
MIN_RUNS=5

# Test repositories: name|repo|branch|glob_pattern|search_dir
REPOS=(
  "bootstrap|twbs/bootstrap|main|scss/**/*.scss|scss"
  "gutenberg|wordpress/gutenberg|trunk|packages/**/*.scss|packages"
)

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m'

info()    { echo -e "${BLUE}==>${NC} ${BOLD}$*${NC}"; }
success() { echo -e "${GREEN}==>${NC} ${BOLD}$*${NC}"; }
warn()    { echo -e "${YELLOW}warning:${NC} $*"; }
error()   { echo -e "${RED}error:${NC} $*"; exit 1; }

# Detect package manager: prefer bun, fall back to npm
detect_pm() {
  local dir="$1"
  if command -v bun &>/dev/null; then
    echo "bun"
  elif [ -f "$dir/pnpm-lock.yaml" ] && command -v pnpm &>/dev/null; then
    echo "pnpm"
  elif [ -f "$dir/yarn.lock" ] && command -v yarn &>/dev/null; then
    echo "yarn"
  else
    echo "npm"
  fi
}

install_deps() {
  local dir="$1"
  if [ -d "$dir/node_modules" ]; then
    echo "    node_modules already present, skipping install"
    return 0
  fi

  local pm
  pm=$(detect_pm "$dir")
  echo "    Installing dependencies with $pm..."

  case "$pm" in
    bun)   (cd "$dir" && bun install --ignore-scripts 2>&1 | tail -1) ;;
    pnpm)  (cd "$dir" && pnpm install --ignore-scripts --no-frozen-lockfile 2>&1 | tail -1) ;;
    yarn)  (cd "$dir" && yarn install --mode skip-build 2>&1 | tail -1) ;;
    npm)   (cd "$dir" && npm install --ignore-scripts 2>&1 | tail -1) ;;
  esac
}

count_files() {
  local dir="$1"
  local pattern="$2"
  # Use find to count matching files (portable)
  local search_subdir
  search_subdir=$(echo "$pattern" | cut -d'/' -f1)
  find "$dir/$search_subdir" -name "*.scss" -o -name "*.css" 2>/dev/null | grep -v node_modules | wc -l | tr -d ' '
}

# ---------------------------------------------------------------------------
# Prerequisites check
# ---------------------------------------------------------------------------

check_prereqs() {
  info "Checking prerequisites..."

  local missing=0

  if ! command -v cargo &>/dev/null; then
    warn "cargo not found. Install Rust: https://rustup.rs"
    missing=1
  fi

  if ! command -v node &>/dev/null; then
    warn "node not found. Install Node.js >= 18: https://nodejs.org"
    missing=1
  fi

  if ! command -v hyperfine &>/dev/null; then
    warn "hyperfine not found. Install it:"
    echo "    macOS:  brew install hyperfine"
    echo "    Linux:  cargo install hyperfine  (or apt/dnf)"
    echo "    Other:  https://github.com/sharkdp/hyperfine#installation"
    missing=1
  fi

  if ! command -v python3 &>/dev/null; then
    warn "python3 not found. Required for result parsing and parity tests."
    missing=1
  fi

  if ! command -v git &>/dev/null; then
    warn "git not found."
    missing=1
  fi

  if [ "$missing" -eq 1 ]; then
    error "Missing prerequisites. Install them and re-run."
  fi

  success "All prerequisites found"
}

# ---------------------------------------------------------------------------
# Build Gale
# ---------------------------------------------------------------------------

build_gale() {
  if [ -f "$GALE_BIN" ]; then
    info "Gale release binary already built (use 'cargo build --release' to rebuild)"
  else
    info "Building Gale in release mode..."
    (cd "$PROJECT_DIR" && cargo build --release)
    success "Build complete"
  fi

  echo "    Binary: $GALE_BIN"
  echo "    Version: $("$GALE_BIN" --version 2>/dev/null || echo 'unknown')"
}

# ---------------------------------------------------------------------------
# Clone repos
# ---------------------------------------------------------------------------

clone_repo() {
  local name="$1" repo="$2" branch="$3"
  local dest="$CLONES_DIR/$name"

  if [ -d "$dest" ]; then
    echo "    [skip] $name already cloned"
    return 0
  fi

  echo "    [clone] $repo @ $branch"
  git clone --depth 1 --branch "$branch" "https://github.com/$repo.git" "$dest" 2>&1 | tail -1
}

# ---------------------------------------------------------------------------
# Run benchmarks
# ---------------------------------------------------------------------------

run_benchmark_for_repo() {
  local name="$1" repo="$2" branch="$3" glob_pattern="$4" search_dir="$5"
  local clone_dir="$CLONES_DIR/$name"
  local stylelint_bin="$clone_dir/node_modules/.bin/stylelint"
  local hyperfine_json="$SCRIPT_DIR/.hyperfine-${name}.json"

  info "Benchmarking: $name"

  # Clone
  clone_repo "$name" "$repo" "$branch"

  # Install deps
  install_deps "$clone_dir"

  # Check stylelint is available
  if [ ! -f "$stylelint_bin" ]; then
    warn "Stylelint not found in $name's node_modules. Skipping."
    echo "$name|0|SKIP|SKIP|SKIP" >> "$SCRIPT_DIR/.benchmark-results.txt"
    return 0
  fi

  # Count files
  local file_count
  file_count=$(count_files "$clone_dir" "$glob_pattern")
  echo "    Files matching pattern: $file_count"

  # Run hyperfine
  info "Running hyperfine ($MIN_RUNS runs, $WARMUP warmup)..."

  hyperfine \
    --warmup "$WARMUP" \
    --min-runs "$MIN_RUNS" \
    --export-json "$hyperfine_json" \
    --command-name "stylelint" \
    "cd $clone_dir && $stylelint_bin '$glob_pattern' --quiet 2>/dev/null; true" \
    --command-name "gale" \
    "cd $clone_dir && $GALE_BIN '$glob_pattern' --quiet 2>/dev/null; true"

  # Parse results from JSON
  local stylelint_mean gale_mean speedup
  stylelint_mean=$(python3 -c "
import json, sys
with open('$hyperfine_json') as f:
    data = json.load(f)
for r in data['results']:
    if r['command'] == 'stylelint':
        print(f\"{r['mean']:.3f}\")
" 2>/dev/null || echo "N/A")

  gale_mean=$(python3 -c "
import json, sys
with open('$hyperfine_json') as f:
    data = json.load(f)
for r in data['results']:
    if r['command'] == 'gale':
        print(f\"{r['mean']:.3f}\")
" 2>/dev/null || echo "N/A")

  if [ "$stylelint_mean" != "N/A" ] && [ "$gale_mean" != "N/A" ]; then
    speedup=$(python3 -c "print(f'{$stylelint_mean / $gale_mean:.1f}')" 2>/dev/null || echo "?")
  else
    speedup="?"
  fi

  echo "$name|$file_count|${stylelint_mean}s|${gale_mean}s|${speedup}x" >> "$SCRIPT_DIR/.benchmark-results.txt"

  success "$name: Gale is ${speedup}x faster (${gale_mean}s vs ${stylelint_mean}s)"
}

# ---------------------------------------------------------------------------
# Run parity (differential) test
# ---------------------------------------------------------------------------

run_parity_test() {
  local name="$1" repo="$2" branch="$3" glob_pattern="$4" search_dir="$5"
  local clone_dir="$CLONES_DIR/$name"
  local stylelint_bin="$clone_dir/node_modules/.bin/stylelint"

  if [ ! -f "$stylelint_bin" ]; then
    echo "$name|SKIP|SKIP|SKIP" >> "$SCRIPT_DIR/.parity-results.txt"
    return 0
  fi

  info "Parity test: $name"

  # Run both linters with JSON output, save to temp files to avoid
  # shell quoting issues with embedded JSON
  local stylelint_tmp="$SCRIPT_DIR/.parity-stylelint-${name}.json"
  local gale_tmp="$SCRIPT_DIR/.parity-gale-${name}.json"

  (cd "$clone_dir" && "$stylelint_bin" "$glob_pattern" --formatter json --quiet 2>/dev/null || true) > "$stylelint_tmp"
  (cd "$clone_dir" && "$GALE_BIN" "$glob_pattern" --formatter json --quiet 2>/dev/null || true) > "$gale_tmp"

  # Compare using Python for robust JSON diffing
  local parity_result
  parity_result=$(python3 - "$stylelint_tmp" "$gale_tmp" <<'PYEOF'
import json, sys

def parse_warnings(path):
    """Extract (file, line, column, rule) tuples from linter JSON output."""
    try:
        with open(path) as f:
            text = f.read().strip()
        if not text:
            return set()
        data = json.loads(text)
    except (json.JSONDecodeError, FileNotFoundError):
        return set()
    warnings = set()
    for entry in data:
        source = entry.get("source", "")
        for w in entry.get("warnings", []):
            rule = w.get("rule", "")
            line = w.get("line", 0)
            col = w.get("column", 0)
            warnings.add((source, line, col, rule))
    return warnings

stylelint_w = parse_warnings(sys.argv[1])
gale_w = parse_warnings(sys.argv[2])

# Only compare rules that Gale implements
gale_rules = {
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

stylelint_filtered = {w for w in stylelint_w if w[3] in gale_rules}
gale_filtered = {w for w in gale_w if w[3] in gale_rules}

false_negatives = stylelint_filtered - gale_filtered
false_positives = gale_filtered - stylelint_filtered

total_files = len({w[0] for w in stylelint_filtered | gale_filtered})

print(f"{total_files}|{len(false_positives)}|{len(false_negatives)}")
PYEOF
  ) || parity_result="ERR|ERR|ERR"

  rm -f "$stylelint_tmp" "$gale_tmp"

  local total_files fp fn
  total_files=$(echo "$parity_result" | cut -d'|' -f1)
  fp=$(echo "$parity_result" | cut -d'|' -f2)
  fn=$(echo "$parity_result" | cut -d'|' -f3)

  echo "$name|$total_files|$fp|$fn" >> "$SCRIPT_DIR/.parity-results.txt"
  echo "    Files tested: $total_files | False positives: $fp | False negatives: $fn"
}

# ---------------------------------------------------------------------------
# Generate results markdown
# ---------------------------------------------------------------------------

generate_results() {
  info "Generating results..."

  local date_str
  date_str=$(date -u +"%Y-%m-%d %H:%M UTC")

  cat > "$RESULTS_FILE" <<EOF
# Benchmark Results

> Generated on $date_str
> System: $(uname -s) $(uname -m) | $(uname -r)
> Node: $(node --version 2>/dev/null || echo 'N/A') | Rust: $(rustc --version 2>/dev/null | cut -d' ' -f2 || echo 'N/A')

## Performance

| Repository | Files | Stylelint | Gale | Speedup |
|------------|------:|----------:|-----:|--------:|
EOF

  while IFS='|' read -r name files stylelint_time gale_time speedup; do
    echo "| $name | $files | $stylelint_time | $gale_time | $speedup |" >> "$RESULTS_FILE"
  done < "$SCRIPT_DIR/.benchmark-results.txt"

  cat >> "$RESULTS_FILE" <<'EOF'

## Parity (Correctness)

| Repository | Files Tested | False Positives | False Negatives |
|------------|-------------:|----------------:|----------------:|
EOF

  while IFS='|' read -r name total_files fp fn; do
    echo "| $name | $total_files | $fp | $fn |" >> "$RESULTS_FILE"
  done < "$SCRIPT_DIR/.parity-results.txt"

  cat >> "$RESULTS_FILE" <<'EOF'

---

*False Positives = Gale reports but Stylelint does not. False Negatives = Stylelint reports but Gale misses.*
*Only rules implemented in Gale are compared. Plugin-only rules are excluded.*

Reproduce these results: `./benchmarks/benchmark.sh`
EOF

  success "Results written to $RESULTS_FILE"
  echo ""
  cat "$RESULTS_FILE"
}

# ---------------------------------------------------------------------------
# Cleanup temp files
# ---------------------------------------------------------------------------

cleanup_temp() {
  rm -f "$SCRIPT_DIR/.benchmark-results.txt"
  rm -f "$SCRIPT_DIR/.parity-results.txt"
  rm -f "$SCRIPT_DIR"/.hyperfine-*.json
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

usage() {
  echo "Usage: $0 [OPTIONS] [REPO...]"
  echo ""
  echo "Run Gale vs Stylelint benchmarks on real-world repositories."
  echo ""
  echo "Repos:    bootstrap, gutenberg (default: all)"
  echo ""
  echo "Options:"
  echo "  --help          Show this help"
  echo "  --skip-build    Skip building Gale (use existing binary)"
  echo "  --skip-parity   Skip the parity/correctness test"
  echo "  --clean         Remove cloned repos and start fresh"
  echo ""
  echo "Prerequisites: cargo, node (>=18), hyperfine, git, python3"
}

main() {
  local skip_build=0
  local skip_parity=0
  local clean=0
  local selected_repos=()

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --help|-h)    usage; exit 0 ;;
      --skip-build) skip_build=1 ;;
      --skip-parity) skip_parity=1 ;;
      --clean)      clean=1 ;;
      -*)           error "Unknown option: $1" ;;
      *)            selected_repos+=("$1") ;;
    esac
    shift
  done

  echo ""
  echo "============================================"
  echo "  GALE vs STYLELINT -- REPRODUCIBLE BENCHMARK"
  echo "============================================"
  echo ""

  # Prerequisites
  check_prereqs

  # Clean if requested
  if [ "$clean" -eq 1 ]; then
    info "Cleaning cloned repos..."
    rm -rf "$CLONES_DIR"
  fi

  mkdir -p "$CLONES_DIR"

  # Build
  if [ "$skip_build" -eq 0 ]; then
    build_gale
  else
    if [ ! -f "$GALE_BIN" ]; then
      error "Gale binary not found at $GALE_BIN. Run without --skip-build first."
    fi
    info "Using existing Gale binary"
  fi

  # Filter repos if specific ones requested
  local repos_to_run=()
  if [ ${#selected_repos[@]} -gt 0 ]; then
    for sel in "${selected_repos[@]}"; do
      for entry in "${REPOS[@]}"; do
        local entry_name
        entry_name=$(echo "$entry" | cut -d'|' -f1)
        if [ "$entry_name" = "$sel" ]; then
          repos_to_run+=("$entry")
        fi
      done
    done
    if [ ${#repos_to_run[@]} -eq 0 ]; then
      error "No matching repos found. Available: bootstrap, gutenberg"
    fi
  else
    repos_to_run=("${REPOS[@]}")
  fi

  # Clean temp files
  cleanup_temp

  echo ""

  # Run benchmarks
  for entry in "${repos_to_run[@]}"; do
    IFS='|' read -r name repo branch glob_pattern search_dir <<< "$entry"
    run_benchmark_for_repo "$name" "$repo" "$branch" "$glob_pattern" "$search_dir"
    echo ""
  done

  # Run parity tests
  if [ "$skip_parity" -eq 0 ]; then
    for entry in "${repos_to_run[@]}"; do
      IFS='|' read -r name repo branch glob_pattern search_dir <<< "$entry"
      run_parity_test "$name" "$repo" "$branch" "$glob_pattern" "$search_dir"
      echo ""
    done
  fi

  # Generate results
  generate_results

  # Cleanup
  cleanup_temp

  echo ""
  success "Done! Results saved to benchmarks/results.md"
}

main "$@"
