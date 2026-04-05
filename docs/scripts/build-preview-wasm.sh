#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
DOCS_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
REPO_DIR="$(cd -- "$DOCS_DIR/.." && pwd)"
OUT_DIR="$DOCS_DIR/public/wasm"
TARGET="wasm32-wasip1"
ARTIFACT_DIR="$REPO_DIR/target/$TARGET/release"

PACKAGES=(
  counter-example
  minesweeper-example
)

EXAMPLE_BINS=(
  counter
  minesweeper
)

PREVIEW_BINS=(
  preview-counter
  preview-minesweeper
)

echo "Building wasm previews (${TARGET})..."
cargo build \
  --offline \
  --manifest-path "$REPO_DIR/Cargo.toml" \
  --target "$TARGET" \
  --release \
  --package "${PACKAGES[0]}" \
  --package "${PACKAGES[1]}" \
  --bins

mkdir -p "$OUT_DIR"
rm -f "$OUT_DIR"/preview-*.wasm
for i in "${!EXAMPLE_BINS[@]}"; do
  src="$ARTIFACT_DIR/${EXAMPLE_BINS[$i]}.wasm"
  dst="$OUT_DIR/${PREVIEW_BINS[$i]}.wasm"
  cp "$src" "$dst"
  echo "Wrote $dst"
done
