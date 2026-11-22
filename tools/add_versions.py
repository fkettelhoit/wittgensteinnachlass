#!/usr/bin/env python3
"""Add links to other versions of each remark (from similar/similar.tsv) to the
document markdown, for display in the margin next to the facsimile links.

Usage: add_versions.py <md-dir> <similar.tsv> <out-dir>

Writes a copy of every Ms-*/Ts-* document and W-* work from <md-dir> to <out-dir>,
with an HTML block inserted at the start of each remark that has versions (in works,
the versions of the remark's source remark):

    <nav class="versions"><a …>←172</a> <details><summary>+4</summary><a …>≈213</a>…</details></nav>

The first entry of the remark's chronology is always visible; the rest are behind
`+N`. Entries are ordered earlier (←), concurrent (≈), later (→), and by date within
each group. Labels drop the Ms-/Ts- prefix; a document that occurs more than once in
a remark's list gets a superscript index. The page template moves the block from the
remark body into the heading, i.e. the margin column with the work references.

The block goes into the body rather than the markdown heading because headings
without an explicit {#id} get their anchor from the heading text, which must not
change. The template moves it into the rendered heading after the anchors exist.
"""

import csv
import os
import re
import sys
from collections import defaultdict

MIN_SCORE = 0.7

FAC_SPAN = re.compile(r'<span class="fac">(.*?)</span>')
FAC_LINK = re.compile(r"\[((?:\\.|[^\]\\])*)\]\((https?://[^)]+)\)")
WORK_SOURCE = re.compile(r"^### \[((?:Ms|Ts)-[^\]]+)\]\(/")
EXPLICIT_ID = re.compile(r"\{#([^}]+)\}\s*$")

ARROWS = {"earlier": "←", "concurrent": "≈", "later": "→"}
TITLES = {"earlier": "Earlier version", "concurrent": "Concurrent version", "later": "Later version"}
GROUP_ORDER = {"earlier": 0, "concurrent": 1, "later": 2}


def github_slug(text):
    """Hugo's default heading ID (see tools/search-index/src/slug.rs)."""
    out, pending = [], False
    for c in text:
        if c.isalnum():
            if pending and out:
                out.append("-")
            pending = False
            out.append(c.lower())
        elif c == "_":
            if pending and out:
                out.append("-")
            pending = False
            out.append("_")
        elif c in " -":
            pending = True
    return "".join(out)


def date_base(siglum):
    """Strip the `.N` suffix of remarks split across work entries."""
    m = re.match(r"^(.*\])\.\d+$", siglum)
    return m.group(1) if m else siglum


def parse_remarks(md_dir, docs):
    """siglum → dict(doc, fragment, date_key, pos), mirroring tools/similar (sigla)
    and tools/search-index (fragments)."""
    remarks = {}
    pos = 0
    for doc in docs:
        seen_ids = {}
        seen_sigla = defaultdict(int)
        with open(os.path.join(md_dir, doc + ".md"), encoding="utf-8") as f:
            for line in f:
                if not line.startswith("### "):
                    continue
                heading = line[4:].rstrip("\n")
                fac = FAC_SPAN.search(heading)
                labels = [
                    m.group(1).replace("\\[", "[").replace("\\]", "]")
                    for m in FAC_LINK.finditer(fac.group(1) if fac else heading)
                ]
                # Anchor, exactly as Hugo assigns it.
                explicit = EXPLICIT_ID.search(heading)
                if explicit:
                    fragment = explicit.group(1)
                    seen_ids[fragment] = max(seen_ids.get(fragment, 0), 1)
                else:
                    base = github_slug("".join(labels))
                    if not base:
                        fragment = None
                    else:
                        count = seen_ids.get(base, 0)
                        fragment = base if count == 0 else f"{base}-{count}"
                        seen_ids[base] = count + 1
                # Siglum, exactly as tools/similar assigns it.
                siglum = f"{doc},{'et'.join(labels)}"
                seen_sigla[siglum] += 1
                n = seen_sigla[siglum]
                key = (date_base(siglum), n - 1)
                if n > 1:
                    siglum = f"{siglum}-{n}"
                remarks[siglum] = {"doc": doc, "fragment": fragment, "date_key": key, "pos": pos}
                pos += 1
    return remarks


def load_dates(path):
    dates = defaultdict(list)
    with open(path, encoding="utf-8") as f:
        for row in csv.DictReader(f, delimiter="\t"):
            dates[row["siglum"]].append((row["start"], row["end"]) if row["start"] else None)
    return dates


def load_versions(path):
    """siglum → [(other siglum, group)] for pairs at or above MIN_SCORE."""
    versions = defaultdict(list)
    with open(path, encoding="utf-8") as f:
        for row in csv.DictReader(f, delimiter="\t"):
            if float(row["score"]) < MIN_SCORE:
                continue
            a, b, order = row["a"], row["b"], row["order"]
            if order == "<":
                versions[a].append((b, "later"))
                versions[b].append((a, "earlier"))
            elif order == ">":
                versions[a].append((b, "earlier"))
                versions[b].append((a, "later"))
            else:
                versions[a].append((b, "concurrent"))
                versions[b].append((a, "concurrent"))
    return versions


def short_name(doc):
    return re.sub(r"^(Ms|Ts)-", "", doc)


def nav_html(entries, remarks, dates, first_line=False):
    def date_of(siglum):
        base, occurrence = remarks[siglum]["date_key"]
        ranges = dates.get(base, [])
        d = ranges[occurrence] if occurrence < len(ranges) else (ranges[0] if ranges else None)
        return d or ("99999999", "99999999")

    entries = [(s, g) for s, g in entries if s in remarks and remarks[s]["fragment"]]
    entries.sort(key=lambda e: (GROUP_ORDER[e[1]], date_of(e[0]), remarks[e[0]]["pos"]))
    if not entries:
        return None

    per_doc = defaultdict(int)
    for s, _ in entries:
        per_doc[remarks[s]["doc"]] += 1
    index = defaultdict(int)
    links = []
    for s, group in entries:
        r = remarks[s]
        label = ARROWS[group] + short_name(r["doc"])
        if per_doc[r["doc"]] > 1:
            index[r["doc"]] += 1
            label += f"<sup>{index[r['doc']]}</sup>"
        href = f"/{r['doc'].lower()}/#{r['fragment']}"
        links.append(f'<a href="{href}" title="{TITLES[group]}: {s}">{label}</a>')

    # Without work references above it, the block starts on the heading's first line.
    cls = "versions first-line" if first_line else "versions"
    html = f'<nav class="{cls}">' + links[0]
    if len(links) > 1:
        # One entry per line in the margin column (<br>), but real spaces too, so the
        # list can flow and wrap on narrow screens, where the CSS hides the <br>s.
        rest = "".join(f"<br> {link}" for link in links[1:])
        html += f" <details><summary>+{len(links) - 1}</summary>{rest}</details>"
    return html + "</nav>"


def main():
    md_dir, similar_path, out_dir = sys.argv[1:4]
    docs = sorted(
        f[:-3] for f in os.listdir(md_dir) if re.match(r"^(Ms|Ts)-.*\.md$", f)
    )
    remarks = parse_remarks(md_dir, docs)
    dates = load_dates(os.path.join(md_dir, "dates.tsv"))
    versions = load_versions(similar_path)

    works = sorted(f[:-3] for f in os.listdir(md_dir) if re.match(r"^W-.*\.md$", f))

    os.makedirs(out_dir, exist_ok=True)
    total = 0
    for name in docs + works:
        is_work = name.startswith("W-")
        seen_sigla = defaultdict(int)
        out = []
        with open(os.path.join(md_dir, name + ".md"), encoding="utf-8") as f:
            for line in f:
                out.append(line)
                if not line.startswith("### "):
                    continue
                fac = FAC_SPAN.search(line)
                labels = [
                    m.group(1).replace("\\[", "[").replace("\\]", "]")
                    for m in FAC_LINK.finditer(fac.group(1) if fac else line)
                ]
                if is_work:
                    # A work remark is its source remark: `### [Ts-227a](/ts-227a/#1.1) …`
                    source = WORK_SOURCE.match(line)
                    if not source:
                        continue
                    siglum = f"{source.group(1)},{'et'.join(labels)}"
                else:
                    siglum = f"{name},{'et'.join(labels)}"
                    seen_sigla[siglum] += 1
                    if seen_sigla[siglum] > 1:
                        siglum = f"{siglum}-{seen_sigla[siglum]}"
                # A heading that starts with a link has references in the work column:
                # works on document pages, the source document on work pages.
                first_line = not line.startswith("### [")
                html = nav_html(versions.get(siglum, []), remarks, dates, first_line)
                if html:
                    # Markdown: the heading is followed by a blank line, then the HTML block.
                    out.append("\n" + html + "\n")
                    total += 1
        with open(os.path.join(out_dir, name + ".md"), "w", encoding="utf-8") as f:
            f.write("".join(out))
    print(f"Added versions to {total} remarks.", file=sys.stderr)


if __name__ == "__main__":
    main()
