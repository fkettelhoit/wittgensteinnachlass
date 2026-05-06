#!/usr/bin/env python3
"""Merge German and English markdown files into a bilingual side-by-side format.

Splits both files by ### (h3) section markers, then outputs each section with
the German and English content wrapped in <div class="de"> and <div class="en">.
Blank lines around div tags are required for Goldmark to process markdown inside.
"""

import re
import sys


def split_sections(text):
    """Split markdown text into sections by ### headings."""
    return re.split(r"(?=^### )", text, flags=re.MULTILINE)


def main():
    if len(sys.argv) != 3:
        print(f"Usage: {sys.argv[0]} <german.md> <english.md>", file=sys.stderr)
        sys.exit(1)

    de_text = open(sys.argv[1]).read()
    en_text = open(sys.argv[2]).read()

    de_parts = split_sections(de_text)
    en_parts = split_sections(en_text)

    # First part is the preamble (# Title + any content before first ###)
    de_preamble = de_parts[0] if de_parts else ""
    en_preamble = en_parts[0] if en_parts else ""

    de_sections = de_parts[1:] if len(de_parts) > 1 else []
    en_sections = en_parts[1:] if len(en_parts) > 1 else []

    # Output the German preamble: title line first, then any remaining
    # preamble content (e.g. <details> with viz) wrapped in a .de div
    preamble_lines = de_preamble.rstrip("\n").split("\n")
    # Find the title line (# ...)
    title_end = 0
    for j, line in enumerate(preamble_lines):
        if line.startswith("# "):
            title_end = j + 1
            break
    sys.stdout.write("\n".join(preamble_lines[:title_end]) + "\n")
    remaining = "\n".join(preamble_lines[title_end:]).strip()
    if remaining:
        sys.stdout.write(f'\n<div class="preamble">\n\n{remaining}\n\n</div>\n')

    count = max(len(de_sections), len(en_sections))
    for i in range(count):
        de_sec = de_sections[i] if i < len(de_sections) else ""
        en_sec = en_sections[i] if i < len(en_sections) else ""

        # Extract heading line and body from each section
        de_lines = de_sec.split("\n", 1)
        en_lines = en_sec.split("\n", 1)

        heading = de_lines[0]  # Use German heading (identical in both)
        de_body = de_lines[1].strip() if len(de_lines) > 1 else ""
        en_body = en_lines[1].strip() if len(en_lines) > 1 else ""

        sys.stdout.write(f"\n{heading}\n\n")
        sys.stdout.write(f'<div class="de">\n\n{de_body}\n\n</div>\n')
        sys.stdout.write(f'<div class="en">\n\n{en_body}\n\n</div>\n')


if __name__ == "__main__":
    main()
