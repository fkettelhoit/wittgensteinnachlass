#!/usr/bin/env python3
"""Post-build gate: fail if any rendered page shows a Markdown-rendering defect.

Scans site/public/**/index.html against the md/ sources for three signatures of
transcription notation colliding with Markdown (see tools/sanitize_content.py):
  1. truncation     — rendered <h3> count != source '### ' count (a stray code
                      fence / construct swallowed the rest of the doc)
  2. math corruption — emphasis/code tags leaked inside a <math>...</math> span
  3. blockquote      — a literal '>' line rendered as <blockquote>

Exits non-zero (with a per-doc report) on any defect. Run via `make check-render`.
"""

import glob
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MD_DIR = os.path.join(ROOT, "md")
PUBLIC = os.path.join(ROOT, "site", "public")

NON_DOC = {"index", "all", "docs-by-date", "docs-by-name"}
# Layout-only docs with zero '### ' remarks but a single layout <h3>.
H3_ALLOWLIST = {"ts-309-stonborough", "ts-310-redpath"}

MATH_SPAN = re.compile(r"<math\b[^>]*>(.*?)</math>", re.DOTALL)
LEAK_TAGS = ("<em>", "<strong>", "<code>", "<del>")


def source_h3_counts():
    counts = {}
    for src in glob.glob(os.path.join(MD_DIR, "*.md")):
        stem = os.path.basename(src)[:-3]
        if stem in NON_DOC:
            continue
        with open(src) as f:
            counts[stem.lower()] = sum(1 for line in f if line.startswith("### "))
    return counts


def check_page(path, slug, src_h3):
    html = open(path).read()
    problems = []
    if slug not in H3_ALLOWLIST and slug in src_h3:
        rendered = html.count("<h3")
        if rendered != src_h3[slug]:
            problems.append(f"truncation: rendered <h3>={rendered} vs source ###={src_h3[slug]}")
    bad_spans = sum(1 for m in MATH_SPAN.finditer(html) if any(t in m.group(1) for t in LEAK_TAGS))
    if bad_spans:
        problems.append(f"math corruption: {bad_spans} <math> span(s) with leaked emphasis/code")
    if "<blockquote" in html:
        problems.append(f"blockquote: {html.count('<blockquote')} unexpected <blockquote>")
    return problems


def main():
    src_h3 = source_h3_counts()
    failures = {}
    for path in sorted(glob.glob(os.path.join(PUBLIC, "**", "index.html"), recursive=True)):
        slug = os.path.basename(os.path.dirname(path))
        if slug not in src_h3:  # only manuscript/work doc pages (de + en/)
            continue
        problems = check_page(path, slug, src_h3)
        if problems:
            failures[os.path.relpath(path, PUBLIC)] = problems

    if failures:
        print(f"check-render FAILED: {len(failures)} page(s) with rendering defects\n")
        for page, problems in failures.items():
            print(f"  {page}")
            for p in problems:
                print(f"    - {p}")
        sys.exit(1)
    print(f"check-render OK: {sum(1 for _ in glob.glob(os.path.join(PUBLIC, '**', 'index.html'), recursive=True))} pages scanned, no defects")


if __name__ == "__main__":
    main()
