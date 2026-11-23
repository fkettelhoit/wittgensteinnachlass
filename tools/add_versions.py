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

Documents with versions of their remarks in other documents also get a collapsible
summary right after the document's own `<details>` block, with a nested summary and
visualization (tools/visualize --all-similar) for each other document with at least
MIN_SIMILAR_VIZ of them, most first, and the remaining documents on one line:

    <details class="similar"><summary …>32 similar remarks in <a …>Ts-211</a>, …</summary>
    <details><summary …>20 similar in <a …>Ts-211</a></summary><img class="viz" …></details>
    <details><summary …>10 similar in <a …>Ts-212</a></summary><img class="viz" …></details>
    <p>and 2 in <a href="/ts-213/">Ts-213</a></p>
    </details>

The names in these summaries, and in the parser's summary of the document's or work's
own visualization, are links to the documents and works (see linked_summary).

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
# Minimum number of a document's remarks with versions in another document for a
# visualization of the pair (tools/visualize: MIN_SIMILAR_VIZ).
MIN_SIMILAR_VIZ = 10

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


def similar_summary(doc, sigla, versions, remarks):
    """The `<details>` block for a document's similar remarks, or None.

    The other documents (pairs within the document are left out) are ordered by the
    number of this document's remarks with versions in them, most first. Each one with
    at least MIN_SIMILAR_VIZ such remarks gets a nested `<details>` with the
    visualization of this document against it; the rest follow on a single line.
    Must agree with generate_similar_svgs in tools/visualize on which pairs get one."""
    count = 0
    per_target = {}  # other document → number of this document's remarks
    for siglum in sigla:
        docs = {remarks[s]["doc"] for s, _ in versions.get(siglum, []) if s in remarks} - {doc}
        if not docs:
            continue
        count += 1
        for d in sorted(docs):
            per_target[d] = per_target.get(d, 0) + 1
    if not count:
        return None

    # Most overlap first; ties in order of first appearance (dicts keep insertion order).
    targets = sorted(per_target.items(), key=lambda e: -e[1])
    def href(target):
        return f"/{target.lower()}/"

    # Same cutoff as the parser's summaries of works and documents.
    shown = [t for t, _ in targets] if len(targets) <= 5 else [t for t, _ in targets[:4]]
    more = len(targets) - len(shown)
    word = "remark" if count == 1 else "remarks"

    # Closed entries must not load their images (there can be dozens): loading="lazy".
    with_viz = [(t, n) for t, n in targets if n >= MIN_SIMILAR_VIZ]
    rest = [(t, n) for t, n in targets if n < MIN_SIMILAR_VIZ]
    summary = linked_summary(f"{count} similar {word}", "in", [(t, href(t)) for t in shown], more)
    html = f'\n<details class="similar">\n{summary}\n'
    for target, n in with_viz:
        html += (
            f"<details>\n{linked_summary(f'{n} similar', 'in', [(target, href(target))])}\n"
            f'<img class="viz" src="/viz/{doc}-similar-{target}.svg" loading="lazy">\n</details>\n'
        )
    if rest:
        links = ", ".join(f'{n} in <a href="{href(t)}">{t}</a>' for t, n in rest)
        html += f"<p>{'and ' if with_viz else ''}{links}</p>\n"
    return html + "</details>\n"


INTERNAL_LINK = re.compile(r"\[([^\]]+)\]\((/[^/)]+/)#")
PREAMBLE_SUMMARY = re.compile(r"<summary>(.*? remarks?), (.*?)</summary>")


def linked_summary(toggle, word, links, more=0):
    """A `<summary>` whose text `toggle` opens the details, followed by `word` (plain
    text, e.g. "in") and links to the
    documents or works, as (label, href or None for plain text), and `and N more`. A link inside a summary is
    followed instead of toggling the details; only the toggle text is underlined like a
    summary (CSS: summary.linked)."""
    items = [f'<a href="{href}">{label}</a>' if href else label for label, href in links]
    if more:
        items.append(f"and {more} more")
    return f'<summary class="linked"><span>{toggle}</span> {word} {", ".join(items)}</summary>'


def summary_hrefs(md_dir, names, published):
    """Label → href for the names in the parser's summaries: work labels as the remark
    headings of any file link them (`[RPP II](/w-rpp-2/#…)`), documents by their name.
    Only published documents and works (see published_docs) are included."""
    hrefs = {}
    for name in names:
        with open(os.path.join(md_dir, name + ".md"), encoding="utf-8") as f:
            for line in f:
                if line.startswith("### "):
                    for m in INTERNAL_LINK.finditer(line):
                        hrefs.setdefault(m.group(1), m.group(2))
    for name in names:
        if not name.startswith("W-"):
            hrefs[name] = f"/{name.lower()}/"
    slugs = {f"/{name.lower()}/" for name in published}
    return {label: href for label, href in hrefs.items() if href in slugs}


def link_preamble_summary(text, is_work, hrefs):
    """Turn the names in the summary of the `<details>` block after the title (written
    by the parser: `360 published remarks, PG I, …, and 5 more` on documents, `1933–1934,
    594 remarks, Ms-140, …` on works) into links to the works or source documents.
    Names without a link in `hrefs` (unpublished) stay plain text."""
    first_remark = text.find("\n### ")
    m = PREAMBLE_SUMMARY.search(text)
    if not m or (first_remark != -1 and m.start() > first_remark):
        return text
    names = m.group(2).split(", ")
    more = 0
    if names and (more_m := re.fullmatch(r"and (\d+) more", names[-1])):
        more = int(more_m.group(1))
        names = names[:-1]
    if not names or any(not re.fullmatch(r"[\w\- ]+", n) for n in names):
        return text
    summary = linked_summary(m.group(1), "from" if is_work else "in", [(n, hrefs.get(n)) for n in names], more)
    return text[: m.start()] + summary + text[m.end() :]


def insert_after_preamble_details(text, block):
    """Insert `block` after the `<details>` block that follows the title, if any,
    otherwise after the title line."""
    first_remark = text.find("\n### ")
    end = text.find("</details>\n")
    if end != -1 and (first_remark == -1 or end < first_remark):
        at = end + len("</details>\n")
    else:
        at = text.find("\n") + 1
    return text[:at] + block + text[at:]


def published_docs(md_dir, docs):
    """The documents (or works) the site exposes: those linked from all.md, as in
    tools/search-index and tools/visualize, and the parts of linked works (W-RFM-1 of
    W-RFM). The others have pages too, but are not listed, so nothing links to them.
    All of them if all.md is missing."""
    try:
        with open(os.path.join(md_dir, "all.md"), encoding="utf-8") as f:
            linked = set(re.findall(r"\]\(([^)]+)\.md\)", f.read()))
    except FileNotFoundError:
        print("warning: all.md not found — linking all documents.", file=sys.stderr)
        return set(docs)
    return {d for d in docs if d in linked or any(d.startswith(w + "-") for w in linked if w.startswith("W-"))}


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
    # Only versions in published documents are linked and summarized.
    published = published_docs(md_dir, docs)
    linkable = {s: r for s, r in remarks.items() if r["doc"] in published}
    dates = load_dates(os.path.join(md_dir, "dates.tsv"))
    versions = load_versions(similar_path)

    works = sorted(f[:-3] for f in os.listdir(md_dir) if re.match(r"^W-.*\.md$", f))

    hrefs = summary_hrefs(md_dir, docs + works, published | published_docs(md_dir, works))

    os.makedirs(out_dir, exist_ok=True)
    total = 0
    summaries = 0
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
                html = nav_html(versions.get(siglum, []), linkable, dates, first_line)
                if html:
                    # Markdown: the heading is followed by a blank line, then the HTML block.
                    out.append("\n" + html + "\n")
                    total += 1
        text = link_preamble_summary("".join(out), is_work, hrefs)
        if name in published:
            sigla = [s for s, r in sorted(remarks.items(), key=lambda e: e[1]["pos"]) if r["doc"] == name]
            block = similar_summary(name, sigla, versions, linkable)
            if block:
                text = insert_after_preamble_details(text, block)
                summaries += 1
        with open(os.path.join(out_dir, name + ".md"), "w", encoding="utf-8") as f:
            f.write(text)
    print(f"Added versions to {total} remarks, similar summaries to {summaries} documents.", file=sys.stderr)


if __name__ == "__main__":
    main()
