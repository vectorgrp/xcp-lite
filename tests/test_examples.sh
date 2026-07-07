#!/usr/bin/env bash
# test_examples.sh
# Build and run all xcp-lite examples, capture their generated .a2l files to
# tests/fixtures/, and warn if any A2L changed compared to the stored baseline.
#
# Usage: bash tests/test_examples.sh [--no-build]
#
# Options:
#   --no-build   Skip the cargo build step (use existing binaries)
#
# Note (macOS): 'timeout' is not available; background PID + sleep + kill is used.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
FIXTURES_DIR="$SCRIPT_DIR/fixtures"
TARGET_DIR="$WORKSPACE_DIR/target/debug"

NO_BUILD=false
for arg in "$@"; do
    case "$arg" in
        --no-build) NO_BUILD=true ;;
    esac
done

# Example package names. The A2L file written by each example is <name>.a2l.
EXAMPLES=(
    all_features_demo
    calibration_demo
    hello_xcp
    multi_thread_demo
    single_thread_demo
    struct_measurement_demo
    rayon_demo
    tokio_demo
    point_cloud_demo
)

# ──────────────────────────────────────────────────────────────────────────────
# Locate xcpclient (required)
# ──────────────────────────────────────────────────────────────────────────────
XCPCLIENT_BIN="$(command -v xcpclient 2>/dev/null || true)"
if [[ -z "$XCPCLIENT_BIN" ]]; then
    XCPCLIENT_BIN="$HOME/.cargo/bin/xcpclient"
fi
if [[ ! -x "$XCPCLIENT_BIN" ]]; then
    echo "ERROR: xcpclient not found. Install it with: cargo install xcpclient" >&2
    exit 1
fi

mkdir -p "$FIXTURES_DIR"

# ──────────────────────────────────────────────────────────────────────────────
# 1. Build all examples
# ──────────────────────────────────────────────────────────────────────────────
if [[ "$NO_BUILD" == false ]]; then
    echo "==> Building all examples..."
    cd "$WORKSPACE_DIR"
    cargo build \
        -p all_features_demo \
        -p calibration_demo \
        -p hello_xcp \
        -p multi_thread_demo \
        -p single_thread_demo \
        -p struct_measurement_demo \
        -p rayon_demo \
        -p tokio_demo \
        -p point_cloud_demo
    echo ""
fi

# ──────────────────────────────────────────────────────────────────────────────
# 2. Run each example, collect A2L, compare
# ──────────────────────────────────────────────────────────────────────────────
WARNINGS=()

for PKG in "${EXAMPLES[@]}"; do
    A2L_FILE="$WORKSPACE_DIR/${PKG}.a2l"
    FIXTURE_FILE="$FIXTURES_DIR/${PKG}.a2l"
    BINARY="$TARGET_DIR/$PKG"

    echo "──────────────────────────────────────────────────────────"
    echo "==> $PKG"

    if [[ ! -x "$BINARY" ]]; then
        echo "    WARNING: binary not found: $BINARY  (skipping)"
        WARNINGS+=("$PKG: binary not found, skipped")
        continue
    fi

    # Remove any stale A2L so we can detect whether it was freshly generated.
    rm -f "$A2L_FILE"

    # Start the example from the workspace root (so the A2L lands there).
    cd "$WORKSPACE_DIR"
    "$BINARY" --log-level 0 &
    EXAMPLE_PID=$!

    # Give the server time to initialise, then connect with xcpclient.
    # xcpclient triggers A2L writing on first connect and also checks A2L syntax.
    sleep 0.5
    "$XCPCLIENT_BIN" --udp --log-level 0
    CLIENT_EXIT=$?

    # Terminate the example process.
    kill "$EXAMPLE_PID" 2>/dev/null || true
    wait "$EXAMPLE_PID" 2>/dev/null || true
    echo "    Stopped $PKG (PID $EXAMPLE_PID)"

    if [[ $CLIENT_EXIT -ne 0 ]]; then
        echo "    WARNING: xcpclient exited with code $CLIENT_EXIT for $PKG"
        WARNINGS+=("$PKG: xcpclient failed (exit $CLIENT_EXIT)")
    fi

    # Verify the A2L was actually generated.
    if [[ ! -f "$A2L_FILE" ]]; then
        echo "    WARNING: A2L file was not generated: ${PKG}.a2l  (skipping)"
        WARNINGS+=("$PKG: A2L file was not generated")
        continue
    fi

    # Back up the existing fixture before replacing it.
    if [[ -f "$FIXTURE_FILE" ]]; then
        echo "    Renaming existing fixture -> ${PKG}.a2l.bak"
        mv "$FIXTURE_FILE" "${FIXTURE_FILE}.bak"
    fi

    cp "$A2L_FILE" "$FIXTURE_FILE"
    echo "    Copied ${PKG}.a2l -> tests/fixtures/"

    # Compare the new A2L with the backup and warn on any difference.
    if [[ -f "${FIXTURE_FILE}.bak" ]]; then
        if ! diff -q "$FIXTURE_FILE" "${FIXTURE_FILE}.bak" > /dev/null 2>&1; then
            echo ""
            echo "    *** WARNING: A2L changed for $PKG ***"
            diff --unified=3 "${FIXTURE_FILE}.bak" "$FIXTURE_FILE" || true
            echo ""
            WARNINGS+=("$PKG: A2L file changed (diff shown above)")
        else
            echo "    OK: A2L is unchanged."
        fi
    else
        echo "    No previous fixture found; stored as new baseline."
    fi
done

# ──────────────────────────────────────────────────────────────────────────────
# 3. Clean up generated A2L and JSON files from the workspace root
# ──────────────────────────────────────────────────────────────────────────────
echo ""
echo "==> Cleaning up generated A2L and JSON files from workspace root..."
cd "$WORKSPACE_DIR"
rm -f *.a2l *.a2l.bak *.json
echo "    Done."

# ──────────────────────────────────────────────────────────────────────────────
# 4. Summary
# ──────────────────────────────────────────────────────────────────────────────
echo ""
echo "══════════════════════════════════════════════════════════"
echo "SUMMARY"
echo "══════════════════════════════════════════════════════════"
if [[ ${#WARNINGS[@]} -eq 0 ]]; then
    echo "All A2L files are unchanged. ✓"
    exit 0
else
    echo "Warnings (${#WARNINGS[@]}):"
    for w in "${WARNINGS[@]}"; do
        echo "  - $w"
    done
    echo ""
    echo "If the A2L changes are intentional, delete the corresponding .bak files"
    echo "in tests/fixtures/ to accept the new A2L as the updated baseline."
    exit 1
fi
