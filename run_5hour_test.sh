#!/bin/bash
set -e

echo "╔════════════════════════════════════════════════════════════════════════╗"
echo "║  RT Blockchain / UltraCore-RFT + RiftToken                            ║"
echo "║  Integrated Stress-Fuzzer (5 hours)                                   ║"
echo "║  ✅ Operation history + state fingerprinting + multi-threaded          ║"
echo "╚════════════════════════════════════════════════════════════════════════╝"
echo ""

PROJECT_DIR="$(cd "$(dirname "$0")" && pwd)"
BINARY="$PROJECT_DIR/target/release/fuzz_integrated"

# Check if binary exists
if [ ! -f "$BINARY" ]; then
    echo "📦 Binary not found. Building..."
    cd "$PROJECT_DIR"
    cargo build --release 2>&1 | tail -5
fi

echo ""
echo "🚀 Starting 5-hour fuzzing run..."
echo "   (Press Ctrl+C to stop anytime)"
echo ""

# 5 hours = 21300 seconds
# Use all available threads
$BINARY --seconds 18000 --threads $(nproc 2>/dev/null || echo 4)

RESULT=$?

echo ""
if [ $RESULT -eq 0 ]; then
    echo "✅ SUCCESS: 5-hour run completed without crashes!"
else
    echo "⚠️  Run found a failure (exit code $RESULT)"
fi

exit $RESULT
