#!/bin/bash
# Processes SVGs from output/graphics/ for EPUB embedding:
#   1. Converts text-as-path back to <text> elements (non-gen files with aria-label)
#   2. Sets font-family on all <text> elements
#   3. Crops to content bounding box (removes A4 whitespace)
#   4. Converts text to paths (ensures correct rendering without fonts)
#
# Output goes to output/graphics-cropped/.
# Run from output/tools/graphics/: ./process_graphics.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
OUTPUT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
INPUT_DIR="$OUTPUT_DIR/graphics"
DEST_DIR="$OUTPUT_DIR/graphics-cropped"
PREP_DIR="$(mktemp -d)"
JOBS="${JOBS:-4}"
FONT_FAMILY="serif"

if ! command -v inkscape &>/dev/null; then
  echo "Error: inkscape not found. Install with: brew install --cask inkscape" >&2
  exit 1
fi

rm -rf "$DEST_DIR"
mkdir -p "$DEST_DIR"

# Step 1: Convert text-as-path back to <text> for non-gen SVGs
echo "Step 1: Converting text-as-path back to <text> elements..."
python3 "$SCRIPT_DIR/convert_text_paths.py" \
  --input-dir "$INPUT_DIR" \
  --output-dir "$PREP_DIR" \
  --font "$FONT_FAMILY"

# Step 2: Prepare all SVGs in PREP_DIR
echo "Step 2: Preparing SVGs..."
for src in "$INPUT_DIR"/*.svg; do
  [ -f "$src" ] || continue
  filename="$(basename "$src")"
  prep="$PREP_DIR/$filename"

  if [ ! -f "$prep" ]; then
    # Not converted (no aria-labels, or gen-* file) — copy and inject font on <text>
    sed "s/<text /<text font-family=\"$FONT_FAMILY\" /g" "$src" > "$prep"
  fi
done

# Step 3: Crop + convert text to paths with Inkscape
process_svg() {
  local src="$1"
  local filename
  filename="$(basename "$src")"
  local dst="$DEST_DIR/$filename"

  inkscape "$src" \
    --export-area-drawing \
    --export-text-to-path \
    --export-plain-svg \
    --export-filename="$dst" 2>/dev/null

  echo "  $filename"
}

export -f process_svg
export DEST_DIR

svg_total=$(ls "$PREP_DIR"/*.svg 2>/dev/null | wc -l | tr -d ' ')
echo "Step 3: Cropping $svg_total SVGs with $JOBS parallel jobs..."

find "$PREP_DIR" -name '*.svg' -print0 \
  | xargs -0 -n1 -P"$JOBS" bash -c 'process_svg "$@"' _

# Clean up
rm -rf "$PREP_DIR"

svg_count=$(ls "$DEST_DIR"/*.svg 2>/dev/null | wc -l | tr -d ' ')
echo "Done: $svg_count SVGs in $DEST_DIR/"
