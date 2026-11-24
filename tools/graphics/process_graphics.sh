#!/bin/bash
# Processes SVGs from output/graphics/ for EPUB embedding:
#   1. Sets font-family Pagella on all <text> elements (the parser's gen-* drawings; the
#      lettering of the other drawings is kept as drawn, as outlines)
#   2. Crops to content bounding box (removes A4 whitespace)
#   3. Converts text to paths (ensures correct rendering without fonts)
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
FONT_FAMILY="TeX Gyre Pagella"

if ! command -v inkscape &>/dev/null; then
  echo "Error: inkscape not found. Install with: brew install --cask inkscape" >&2
  exit 1
fi

# Let Inkscape (fontconfig) find Pagella: the system's configuration, wherever this
# platform keeps it, plus the committed fonts. Only the full fonts: tex-gyre/ also has
# texgyrepagella-regular-ascii.otf, an ASCII subset with the same family name, which
# fontconfig may pick (and then drop ü, ß, …).
mkdir -p "$PREP_DIR/fonts"
for style in regular italic bold bolditalic; do
  ln -s "$OUTPUT_DIR/site/fonts/tex-gyre/texgyrepagella-$style.otf" "$PREP_DIR/fonts/"
done
export FONTCONFIG_FILE="$PREP_DIR/fonts.conf"
cat > "$FONTCONFIG_FILE" <<FONTS
<?xml version="1.0"?>
<!DOCTYPE fontconfig SYSTEM "fonts.dtd">
<fontconfig>
  <include ignore_missing="yes">/etc/fonts/fonts.conf</include>
  <include ignore_missing="yes">/opt/homebrew/etc/fonts/fonts.conf</include>
  <include ignore_missing="yes">/usr/local/etc/fonts/fonts.conf</include>
  <dir>/System/Library/Fonts</dir>
  <dir>/Library/Fonts</dir>
  <dir>~/Library/Fonts</dir>
  <dir>$PREP_DIR/fonts</dir>
  <cachedir>$PREP_DIR/fontconfig-cache</cachedir>
</fontconfig>
FONTS

rm -rf "$DEST_DIR"
mkdir -p "$DEST_DIR"
mkdir -p "$PREP_DIR/svg"

# Step 1: Prepare all SVGs, setting the font of any <text>
echo "Step 1: Preparing SVGs..."
for src in "$INPUT_DIR"/*.svg; do
  [ -f "$src" ] || continue
  sed "s/<text /<text font-family=\"$FONT_FAMILY\" /g" "$src" > "$PREP_DIR/svg/$(basename "$src")"
done

# Step 2+3: Crop + convert text to paths with Inkscape
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

svg_total=$(ls "$PREP_DIR"/svg/*.svg 2>/dev/null | wc -l | tr -d ' ')
echo "Step 2: Cropping $svg_total SVGs with $JOBS parallel jobs..."

find "$PREP_DIR/svg" -name '*.svg' -print0 \
  | xargs -0 -n1 -P"$JOBS" bash -c 'process_svg "$@"' _

# Clean up
rm -rf "$PREP_DIR"

svg_count=$(ls "$DEST_DIR"/*.svg 2>/dev/null | wc -l | tr -d ' ')
echo "Done: $svg_count SVGs in $DEST_DIR/"
