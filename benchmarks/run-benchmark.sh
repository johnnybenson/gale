#!/bin/bash
# Benchmark gale vs stylelint on Bootstrap CSS (20x duplicated)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$SCRIPT_DIR/.."
FIXTURES_DIR="$SCRIPT_DIR/fixtures"
BENCHMARK_FILE="$FIXTURES_DIR/bootstrap-20x.css"
BOOTSTRAP_FILE="$FIXTURES_DIR/bootstrap.css"

# Generate fixtures if needed
bash "$SCRIPT_DIR/generate-benchmark.sh"

echo ""
echo "============================================"
echo "  GALE vs STYLELINT BENCHMARK"
echo "============================================"
echo ""

GALE_BIN="$PROJECT_DIR/target/release/gale"

if [ ! -f "$GALE_BIN" ]; then
    echo "Building gale in release mode..."
    (cd "$PROJECT_DIR" && cargo build --release)
fi

# Check if hyperfine is available
if ! command -v hyperfine &>/dev/null; then
    echo "hyperfine not found. Install it: brew install hyperfine"
    echo ""
    echo "Falling back to manual timing..."
    echo ""

    echo "--- Gale (bootstrap.css) ---"
    time "$GALE_BIN" "$BOOTSTRAP_FILE" --quiet 2>/dev/null || true
    echo ""

    echo "--- Gale (bootstrap-20x.css) ---"
    time "$GALE_BIN" "$BENCHMARK_FILE" --quiet 2>/dev/null || true
    echo ""

    if command -v stylelint &>/dev/null; then
        echo "--- Stylelint (bootstrap.css) ---"
        time stylelint "$BOOTSTRAP_FILE" --quiet 2>/dev/null || true
        echo ""

        echo "--- Stylelint (bootstrap-20x.css) ---"
        time stylelint "$BENCHMARK_FILE" --quiet 2>/dev/null || true
    else
        echo "stylelint not found. Install it: bun add -g stylelint stylelint-config-standard"
    fi
    exit 0
fi

# Use hyperfine for proper benchmarking
echo "--- bootstrap.css ($(wc -l < "$BOOTSTRAP_FILE" | tr -d ' ') lines) ---"
echo ""

CMDS=("$GALE_BIN $BOOTSTRAP_FILE --quiet")
NAMES=("gale")

if command -v stylelint &>/dev/null; then
    CMDS+=("stylelint $BOOTSTRAP_FILE --quiet")
    NAMES+=("stylelint")
fi

hyperfine --warmup 3 --min-runs 10 \
    "${CMDS[@]}" \
    --command-name "${NAMES[@]}" \
    2>&1 || true

echo ""
echo "--- bootstrap-20x.css ($(wc -l < "$BENCHMARK_FILE" | tr -d ' ') lines) ---"
echo ""

CMDS_20X=("$GALE_BIN $BENCHMARK_FILE --quiet")
NAMES_20X=("gale")

if command -v stylelint &>/dev/null; then
    CMDS_20X+=("stylelint $BENCHMARK_FILE --quiet")
    NAMES_20X+=("stylelint")
fi

hyperfine --warmup 3 --min-runs 10 \
    "${CMDS_20X[@]}" \
    --command-name "${NAMES_20X[@]}" \
    2>&1 || true
