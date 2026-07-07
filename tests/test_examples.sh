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

# List of "package_name:a2l_base_name" pairs.
# a2l_base_name matches APP_NAME in each example (the server writes <app_name>.a2l
# to the process CWD on finalize_registry() or on first XCP client connection).
EXAMPLES=(
    "all_features_demo:all_features_demo"
    "calibration_demo:cal_demo"
    "hello_xcp:hello_xcp"
    "multi_thread_demo:multi_thread_demo"
    "single_thread_demo:single_thread_demo"
    "struct_measurement_demo:struct_measurement_demo"
    "rayon_demo:rayon_demo"
    "tokio_demo:tokio_demo"
    "point_cloud_demo:point_cloud"
)

# Examples that do NOT call finalize_registry() early and therefore need an XCP
# client connection to trigger A2L writing.
NEEDS_CLIENT=("single_thread_demo" "rayon_demo" "tokio_demo" "point_cloud_demo")

needs_client() {
    local pkg="$1"
    for nc in "${NEEDS_CLIENT[@]}"; do
        [[ "$nc" == "$pkg" ]] && return 0
    done
    return 1
}

# ──────────────────────────────────────────────────────────────────────────────
# Helpers
# ──────────────────────────────────────────────────────────────────────────────

XCPCLIENT_BIN="$(command -v xcpclient 2>/dev/null || true)"
if [[ -z "$XCPCLIENT_BIN" ]]; then
    # Try the well-known cargo-install path
    XCPCLIENT_BIN="$HOME/.cargo/bin/xcpclient"
fi

have_xcpclient() {
    [[ -x "$XCPCLIENT_BIN" ]]
}

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

for entry in "${EXAMPLES[@]}"; do
    PKG="${entry%%:*}"
    A2L_NAME="${entry##*:}"
    A2L_FILE="$WORKSPACE_DIR/${A2L_NAME}.a2l"
    FIXTURE_FILE="$FIXTURES_DIR/${A2L_NAME}.a2l"
    BINARY="$TARGET_DIR/$PKG"

    echo "──────────────────────────────────────────────────────────"
    echo "==> $PKG  (A2L: ${A2L_NAME}.a2l)"

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

    # Give the server time to initialise.
    sleep 0.5

    if needs_client "$PKG"; then
        # These examples only write the A2L upon the first XCP client connection.
        if have_xcpclient; then
            echo "    Connecting xcpclient to trigger A2L write..."
            "$XCPCLIENT_BIN" --udp --log-level 0 &
            CLIENT_PID=$!
            sleep 1
            kill "$CLIENT_PID" 2>/dev/null || true
            wait "$CLIENT_PID" 2>/dev/null || true
        else
            echo "    WARNING: xcpclient not found; cannot trigger A2L write for $PKG"
            WARNINGS+=("$PKG: xcpclient not available, A2L may not be generated")
            sleep 1
        fi
    else
        # finalize_registry() is called during init; give it a moment to finish.
        sleep 1
    fi

    # Terminate the example process.
    kill "$EXAMPLE_PID" 2>/dev/null || true
    wait "$EXAMPLE_PID" 2>/dev/null || true
    echo "    Stopped $PKG (PID $EXAMPLE_PID)"

    # Verify the A2L was actually generated.
    if [[ ! -f "$A2L_FILE" ]]; then
        echo "    WARNING: A2L file was not generated: ${A2L_NAME}.a2l  (skipping)"
        WARNINGS+=("$PKG: A2L file was not generated")
        continue
    fi

    # Back up the existing fixture before replacing it.
    if [[ -f "$FIXTURE_FILE" ]]; then
        echo "    Renaming existing fixture -> ${A2L_NAME}.a2l.bak"
        mv "$FIXTURE_FILE" "${FIXTURE_FILE}.bak"
    fi

    cp "$A2L_FILE" "$FIXTURE_FILE"
    echo "    Copied ${A2L_NAME}.a2l -> tests/fixtures/"

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
# 3. Summary
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
