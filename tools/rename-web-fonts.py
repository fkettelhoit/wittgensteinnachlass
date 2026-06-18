#!/usr/bin/env python3
"""Restore family/style name records on the licensed SangBleu "WebS" TTFs.

The web-subset ("WebS") SangBleu fonts ship with a stripped name table — no usable
family name (family=None, full name '¶', PostScript 'Font'). Browsers and macOS work
anyway because the CSS/SVG @font-face *declares* the family name and maps it to the file.

Headless Linux Inkscape (used by the covers tool) does NOT honour an @font-face
`src: url(file://…)` rule — it resolves fonts via fontconfig by family name. With a
stripped name table, fontconfig can't register the fonts as "SangBleu Empire" /
"SangBleu Sunrise", so the cover titles render with no glyphs. This rewrites the name
records (derived from the filename) so fontconfig — and thus Inkscape — find them.

Usage: rename-web-fonts.py [TTF_DIR]   (default: build/fonts-ttf)
Only the name table is touched; glyphs, cmap and metrics are left intact.
"""
import glob
import os
import re
import sys

from fontTools.ttLib import TTFont

ttf_dir = sys.argv[1] if len(sys.argv) > 1 else "build/fonts-ttf"
renamed = 0
for path in sorted(glob.glob(os.path.join(ttf_dir, "*.ttf"))):
    m = re.match(r"(SangBleu(?:Empire|Sunrise))-(.+?)-WebS", os.path.basename(path))
    if not m:
        continue
    family = "SangBleu Empire" if "Empire" in m.group(1) else "SangBleu Sunrise"
    style = m.group(2)  # e.g. Bold, Regular, RegularItalic
    font = TTFont(path)
    name = font["name"]
    records = [
        (1, family),                                   # Font Family
        (2, style),                                    # Font Subfamily
        (4, f"{family} {style}"),                      # Full name
        (6, f"{family.replace(' ', '')}-{style}"),     # PostScript name
        (16, family),                                  # Typographic Family
        (17, style),                                   # Typographic Subfamily
    ]
    for name_id, value in records:
        name.setName(value, name_id, 3, 1, 0x409)  # Windows / Unicode / en-US
        name.setName(value, name_id, 1, 0, 0)      # Macintosh / Roman / en
    font.save(path)
    print(f"renamed {os.path.basename(path)} -> family={family!r} style={style!r}")
    renamed += 1

if renamed == 0:
    print(f"WARNING: no SangBleu WebS TTFs found in {ttf_dir}", file=sys.stderr)
