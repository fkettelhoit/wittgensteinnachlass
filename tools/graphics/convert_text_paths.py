#!/usr/bin/env python3
"""Convert text-as-path elements in SVGs back to <text> elements.

Uses Inkscape's --query-all to get accurate bounding boxes for each
aria-labeled path/group, then replaces them with <text> elements
positioned at the correct location.

Usage:
    python3 convert_text_paths.py [--input-dir DIR] [--output-dir DIR] [FILE...]

If no files are given, processes all non-generated SVGs in the input directory.
"""

import argparse
import re
import subprocess
from pathlib import Path


def query_bboxes(svg_path):
    """Query Inkscape for bounding boxes of all elements.

    Returns dict of id -> (x, y, width, height) in px.
    """
    try:
        result = subprocess.run(
            ["inkscape", str(svg_path), "--query-all"],
            capture_output=True, text=True, timeout=30,
        )
    except (subprocess.TimeoutExpired, FileNotFoundError):
        return {}

    bboxes = {}
    for line in result.stdout.strip().split("\n"):
        parts = line.split(",")
        if len(parts) == 5:
            try:
                bboxes[parts[0]] = (
                    float(parts[1]), float(parts[2]),
                    float(parts[3]), float(parts[4]),
                )
            except ValueError:
                continue
    return bboxes


def get_viewbox_scale(svg_text):
    """Get the scale factor from px to viewBox units.

    Inkscape queries return px (96 dpi). The SVG viewBox is in user units
    (typically mm for these files: 210mm x 297mm).
    """
    vb_m = re.search(r'viewBox="([\d.]+)\s+([\d.]+)\s+([\d.]+)\s+([\d.]+)"', svg_text)
    if not vb_m:
        return 1.0

    vb_width = float(vb_m.group(3))

    w_m = re.search(r'width="([\d.]+)mm"', svg_text)
    if not w_m:
        return 1.0

    width_mm = float(w_m.group(1))
    px_per_mm = 96.0 / 25.4
    width_px = width_mm * px_per_mm

    return vb_width / width_px


def find_aria_elements(svg_text):
    """Find all elements with aria-label, returning their ids, labels, and font sizes.

    Returns list of (element_id, label, font_size, is_group).
    """
    elements = []

    for m in re.finditer(
        r'<g\b([^>]*?)aria-label="([^"]*)"([^>]*?)>',
        svg_text, re.DOTALL,
    ):
        attrs = m.group(1) + m.group(3)
        label = m.group(2)
        id_m = re.search(r'\bid="([^"]*)"', attrs)
        size_m = re.search(r"font-size:([\d.]+)", m.group(0))
        if id_m and is_convertible_label(label):
            font_size = float(size_m.group(1)) if size_m else 10.0
            elements.append((id_m.group(1), label, font_size, True))

    for m in re.finditer(
        r'<path\b([^>]*?)aria-label="([^"]*)"([^>]*)/?>'  ,
        svg_text, re.DOTALL,
    ):
        attrs = m.group(1) + m.group(3)
        label = m.group(2)
        id_m = re.search(r'\bid="([^"]*)"', attrs)
        size_m = re.search(r"font-size:([\d.]+)", m.group(0))
        if id_m and is_convertible_label(label):
            font_size = float(size_m.group(1)) if size_m else 10.0
            elements.append((id_m.group(1), label, font_size, False))

    return elements


def is_convertible_label(label):
    """Check if a label is safe to convert from path to text.

    Curly braces, brackets used as graphical elements (rotated via transforms),
    and underscore-only labels (horizontal lines) won't render correctly as text.
    """
    stripped = label.strip()
    if not stripped:
        return False
    if all(c in "{}()[]|" for c in stripped):
        return False
    if all(c in "_ " for c in stripped):
        return False
    return True


def convert_svg(svg_path, svg_text, font, scale):
    """Convert text-as-path elements to <text> using Inkscape bboxes.

    Returns (new_svg, count).
    """
    elements = find_aria_elements(svg_text)
    if not elements:
        return svg_text, 0

    bboxes = query_bboxes(svg_path)
    if not bboxes:
        return svg_text, 0

    replacements = {}
    for elem_id, label, font_size, is_group in elements:
        if elem_id not in bboxes:
            continue

        bx, by, bw, bh = bboxes[elem_id]

        x = bx * scale
        y = by * scale
        w = bw * scale
        h = bh * scale

        tx = x
        ty = y + h

        fs = font_size * scale

        label_escaped = (
            label.replace("&", "&amp;")
            .replace("<", "&lt;")
            .replace(">", "&gt;")
        )

        text_elem = (
            f'<text x="{tx:.4f}" y="{ty:.4f}" '
            f'font-size="{fs:.4f}" '
            f'font-family="{font}" '
            f'fill="#000000" '
            f'id="{elem_id}">{label_escaped}</text>'
        )
        replacements[elem_id] = text_elem

    if not replacements:
        return svg_text, 0

    new_svg = svg_text
    count = 0

    def remove_g(match):
        nonlocal count
        g_tag = match.group(1)
        id_m = re.search(r'\bid="([^"]*)"', g_tag)
        if id_m and id_m.group(1) in replacements:
            count += 1
            return ""
        return match.group(0)

    new_svg = re.sub(
        r'(<g\b[^>]*?aria-label="[^"]*"[^>]*>)(.*?)</g>',
        remove_g, new_svg, flags=re.DOTALL,
    )

    def remove_path(match):
        nonlocal count
        full = match.group(0)
        id_m = re.search(r'\bid="([^"]*)"', full)
        if id_m and id_m.group(1) in replacements:
            count += 1
            return ""
        return full

    new_svg = re.sub(
        r'<path\b[^>]*aria-label="[^"]*"[^>]*/>',
        remove_path, new_svg,
    )
    new_svg = re.sub(
        r'<path\b[^>]*aria-label="[^"]*"[^>]*>[^<]*</path>',
        remove_path, new_svg,
    )

    text_block = "\n".join(replacements.values())
    new_svg = new_svg.replace("</svg>", f"{text_block}\n</svg>")

    return new_svg, count


def main():
    parser = argparse.ArgumentParser(
        description="Convert text-as-path to <text> in SVGs using Inkscape bboxes"
    )
    parser.add_argument(
        "files", nargs="*",
        help="SVG files to process (default: all non-gen-* in input dir)",
    )
    parser.add_argument(
        "--font", default="serif",
        help="Font family for <text> elements (default: serif)",
    )
    parser.add_argument(
        "--input-dir", default="../../graphics",
        help="Input directory (default: ../../graphics)",
    )
    parser.add_argument(
        "--output-dir", default="../../graphics-cropped",
        help="Output directory (default: ../../graphics-cropped)",
    )
    args = parser.parse_args()

    if args.files:
        files = [Path(f) for f in args.files]
    else:
        files = sorted(Path(args.input_dir).glob("*.svg"))
        files = [f for f in files if not f.name.startswith("gen-")]

    out_dir = Path(args.output_dir)
    out_dir.mkdir(parents=True, exist_ok=True)

    total_files = 0
    total_paths = 0

    for path in files:
        svg = path.read_text()

        if "aria-label=" not in svg:
            continue

        scale = get_viewbox_scale(svg)
        new_svg, count = convert_svg(path, svg, font=args.font, scale=scale)
        if count == 0:
            continue

        total_files += 1
        total_paths += count

        out_path = out_dir / path.name
        out_path.write_text(new_svg)
        print(f"  [{total_files}] {path.name}: converted {count} text paths")

    print(f"\nConverted {total_paths} text paths in {total_files} files.")
    print(f"Output written to {out_dir}/")


if __name__ == "__main__":
    main()
