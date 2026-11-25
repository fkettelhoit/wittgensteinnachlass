#!/usr/bin/env python3
"""Nachlass Map: every remark of the Nachlass as a small block, as static Hugo pages.

Usage: map.py <md dir> <similar.tsv> <content dir>

Writes <content dir>/map/: the search (_index.html), the map of published remarks
(published.html), one page per theme (themes/<key>.html) and one per major work
(works/<code>.html), plus data.json for the optional scripts (site/static/js/search.js: the
search, orderings by name and format, the bars layout; site/static/js/map.js: links on the
blocks, switching views in place). The pages work without JavaScript: blocks in date order,
with their description as a popover (data-t, shown by CSS). See README.md.
"""

import heapq
import html
import json
import os
import re
import sys
from collections import defaultdict

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.dirname(HERE))
from add_versions import FAC_SPAN, load_min_score, parse_remarks  # noqa: E402
from themes import ARITHMETIC, LATEX, LOGIC_SYMBOLS, THEMES  # noqa: E402

# A remark's shade for a theme: ½ · own evidence + ½ · the share of the WINDOW remarks on
# either side (in the same document) that have evidence.
WINDOW = 5
ROWS = 4  # blocks per column, i.e. per line of the map
# The home page's preview: the beginning of this theme's map, by date, with the pointer on
# this document (its link leads to the theme).
PREVIEW = ("math", "Ts-211")

NOTATION_NAMES = {LATEX: "formulas", ARITHMETIC: "arithmetic", LOGIC_SYMBOLS: "logical notation"}

# --- Remarks -----------------------------------------------------------------------------

WORK_LINK = re.compile(r"^### \[([^\]]+)\]\(/w-")
TAG = re.compile(r"<[^>]+>")
# Formal notation in the running text (LaTeX is recognized by its rendered images).
ARITH = re.compile(r"\d\s*[=+×÷−<>≠≤≥·∙⋅√^]\s*\(?\d|\b[a-zA-Z]\s*[=+×÷−<>≠≤≥]\s*[a-zA-Z0-9]\b(?![a-zäöüß])")
LOGIC = re.compile(r"[∨⊃≡∃∀¬∧⊢φψχξη]|[~∼]\s*[a-zφψ(]|\b[pqr]\s*[.∙·⋅]\s*[pqr~∼]\b"
                   r"|\b[fFφ]\s*\(\s*[a-z]\s*\)|\((?:∃|E)?[xyz]\)")
WORD = re.compile(r"[A-Za-zÄÖÜäöüß]{3,}")
WORD_HYPHEN = re.compile(r"[A-Za-zÄÖÜäöüß][A-Za-zÄÖÜäöüß-]{2,}")


# A token that is part of an expression: figures, operators, symbols and variables (single
# Latin letters, Greek letters: "p", "φx", "(∃x,φ)"), but never two Latin letters in a row,
# which would be a word ("is", "or").
NOTATION_TOKEN = re.compile(r"[^A-Za-zÄÖÜäöüß]*(?:[A-Za-z](?![A-Za-zÄÖÜäöüß])[^A-Za-zÄÖÜäöüß]*)*")


def notation(raw, plain):
    """The kind of formal notation in a remark (or None) and the expression found (None for
    LaTeX, which has no text): the match, extended over the neighbouring tokens that are
    part of the expression ("1 + 1 + 1", "~p . ~q"), without the words around it."""
    if 'class="math' in raw:
        return LATEX, None
    for kind, rx in ((ARITHMETIC, ARITH), (LOGIC_SYMBOLS, LOGIC)):
        m = rx.search(plain)
        if m:
            tokens = [(t.start(), t.end()) for t in re.finditer(r"\S+", plain)]
            inside = [k for k, (a, b) in enumerate(tokens) if a < m.end() and b > m.start()]
            first, last = inside[0], inside[-1]
            part = lambda k: NOTATION_TOKEN.fullmatch(plain[tokens[k][0]:tokens[k][1]])
            while first > 0 and part(first - 1):
                first -= 1
            while last < len(tokens) - 1 and part(last + 1):
                last += 1
            # Cut the first and last token to the expression (no words, quotes or trailing
            # punctuation).
            start = max(tokens[first][0], m.start()) if not part(first) else tokens[first][0]
            end = min(tokens[last][1], m.end()) if not part(last) else tokens[last][1]
            text = re.sub(r"\s+", " ", plain[start:end]).strip(" ,;:.„“”‚‘’\"'")
            while text.endswith(")") and text.count(")") > text.count("("):
                text = text[:-1].rstrip(" ,;:.")
            while text.startswith("(") and text.count("(") > text.count(")"):
                text = text[1:].lstrip()
            if len(text) > 40:  # e.g. a table of figures: its beginning
                text = text[:40].rsplit(" ", 1)[0] + " …"
            return kind, text or m.group(0).strip()
    return None, None


def read_remarks(md_dir):
    """[document] and {document: [remark]} in document order, a remark being a dict with
    siglum, fragment (heading id on the document page), work (code of the edited work it
    is published in, or None), notation and the words and text used for the themes."""
    names = sorted(f[:-3] for f in os.listdir(md_dir) if re.match(r"^(Ms|Ts)-.*\.md$", f))
    sigla = parse_remarks(md_dir, names)
    by_doc = defaultdict(list)
    for siglum, r in sorted(sigla.items(), key=lambda e: e[1]["pos"]):
        by_doc[r["doc"]].append({"siglum": siglum, "fragment": r["fragment"]})
    for name in names:
        bodies, heads = [], []
        with open(os.path.join(md_dir, name + ".md"), encoding="utf-8") as f:
            for line in f:
                if line.startswith("### "):
                    heads.append(line)
                    bodies.append("")
                elif bodies:
                    bodies[-1] += line
        assert len(heads) == len(by_doc[name]), name
        for r, head, body in zip(by_doc[name], heads, bodies):
            m = WORK_LINK.match(head)
            r["work"] = m.group(1).split()[0] if m else None
            plain = TAG.sub(" ", body)
            r["notation"], r["expression"] = notation(body, plain)
            r["text"] = re.sub(r"\s+", " ", plain)
            r["ltext"] = r["text"].lower()
            r["words"] = set(WORD_HYPHEN.findall(plain)) | set(WORD.findall(plain))
            r["lwords"] = {w.lower() for w in r["words"]}
    return names, by_doc


def page_ref(siglum):
    """`Ms-115,118[2]et119[1]` → `Ms-115,118[2]` (the first page reference)."""
    return siglum.split("et")[0]


# --- The home page: major works, orderings, document descriptions -----------------------

def read_index(md_dir):
    idx = open(os.path.join(md_dir, "index.md"), encoding="utf-8").read()
    # Major works: the covers shown directly (not the collapsed "shorter works").
    shown = idx.split('<details class="shorter-works">')[0]
    works = [(code.upper(), html.unescape(title)) for code, title in re.findall(
        r'<a class="cover" href="/w-([a-z0-9]+)/">.*?<span class="cover-info"><span>(.*?)</span>', shown)]

    def ordering(cls):
        start = idx.index(f'class="index doc-list {cls}"')
        end = idx.find('class="index doc-list', start + 10)
        groups = []
        for line in idx[start:end if end > 0 else len(idx)].splitlines():
            h = re.match(r"## ([^{]+?)\s*(\{.*\})?$", line)
            if h:
                groups.append([h.group(1).strip(), []])
            d = re.match(r"### \[([^\]]+)\]", line)
            if d and groups:
                groups[-1][1].append(d.group(1))
        return groups

    orders = {k: ordering("by-" + k) for k in ("date", "name", "format")}
    info = {}
    for d, rest in re.findall(r'^### \[([^\]]+)\]\([^)]*\)(?: <a class="doc-format[^>]*></a>)? <span class="doc-info">(.*)</span>',
                              idx, re.M):
        info.setdefault(d, html.unescape(TAG.sub("", rest)).strip())
    return works, orders, info


# --- Versions: published remarks and work membership, spread to similar remarks -----------

def spread(seeds, edges):
    """Value of every remark reachable from `seeds` (value 1) through similar pairs: the
    best product of the pair scores along a path, and the seed it comes from."""
    val = {i: 1.0 for i in seeds}
    src = {i: i for i in seeds}
    heap = [(-1.0, i) for i in seeds]
    heapq.heapify(heap)
    while heap:
        v, i = heapq.heappop(heap)
        v = -v
        if v < val.get(i, 0):
            continue
        for j, s in edges[i]:
            if v * s > val.get(j, 0) + 1e-9:
                val[j], src[j] = v * s, src[i]
                heapq.heappush(heap, (-v * s, j))
    return val, src


def load_edges(similar_tsv, index):
    """Similar pairs at or above the site-wide minimum score (similar/min-score.txt)."""
    min_score = load_min_score(similar_tsv)
    edges = defaultdict(list)
    with open(similar_tsv, encoding="utf-8") as f:
        next(f)
        for line in f:
            a, b, score = line.split("\t")[:3]
            s = float(score)
            if s >= min_score and a in index and b in index:
                edges[index[a]].append((index[b], s))
                edges[index[b]].append((index[a], s))
    return edges


# --- Themes ------------------------------------------------------------------------------

def compile_term(rx, words, lwords):
    """A term as a function from a remark to what it finds there: [(position, text as
    written)]. Word terms are matched once against every distinct word of the Nachlass
    (`words`, `lwords`: all words, as written and lowercased)."""
    def found(r, forms):
        return [(r["text"].find(w), w) for w in forms]
    for prefix, mode in (("PC:", "phraseC"), ("P:", "phrase"), ("C:", "wordC")):
        if rx.startswith(prefix):
            body = rx[len(prefix):]
            if mode == "wordC":
                hits = {w for w in words if re.fullmatch(body, w)}
                return lambda r: found(r, hits.intersection(r["words"])) if not hits.isdisjoint(r["words"]) else []
            phrase = re.compile(body)
            key = "text" if mode == "phraseC" else "ltext"
            return lambda r: [(m.start(), r["text"][m.start():m.end()]) for m in phrase.finditer(r[key])]
    hits = {w for w in lwords if re.fullmatch(rx, w)}
    return lambda r: (found(r, [w for w in r["words"] if w.lower() in hits])
                      if not hits.isdisjoint(r["lwords"]) else [])


def shades(own):
    """½ · own + ½ · share of the neighbours with evidence, per remark of a document."""
    out = []
    for i in range(len(own)):
        nb = own[max(0, i - WINDOW):i] + own[i + 1:i + 1 + WINDOW]
        out.append(0.5 * own[i] + (0.5 * sum(nb) / len(nb) if nb else 0))
    return out


# --- HTML --------------------------------------------------------------------------------

def level(v, floor):
    """Shade class 0–10 of a value in [0, 1]; values above 0 get at least `floor`."""
    if v <= 0:
        return 0
    return max(1, min(10, round(floor + (10 - floor) * v)))


def short_name(doc):
    return re.sub(r"^(Ms|Ts)-", "", doc)


def render_map(names, by_doc, order, info, cell):
    """The blocks, in the given ordering: a run per document (its sigil, then columns of
    ROWS blocks), a group's label before its first document. `cell(doc, i)` returns the
    block's shade class and title."""
    out = []
    for label, docs in order:
        for k, doc in enumerate(docs):
            if doc not in by_doc:
                continue
            group = f'<b class="g">{html.escape(label)}</b>' if k == 0 else ""
            title = html.escape(f"{doc} {info.get(doc, '')}".strip(), quote=True)
            cells = []
            for i in range(len(by_doc[doc])):
                cls, tip = cell(doc, i)
                cls = f" class=s{cls}" if cls else ""
                cells.append(f'<i{cls} data-t="{html.escape(tip, quote=True)}"></i>')
            cols = "".join("<span class=c>" + "".join(cells[j:j + ROWS]) + "</span>"
                           for j in range(0, len(cells), ROWS))
            out.append(f'<span class="d" data-doc="{doc}">{group}<a class="l" href="/{doc.lower()}/" '
                       f'data-t="{title}">{short_name(doc)}</a>{cols}</span>')
    return " ".join(out)


# A magnifying glass for the Search view (in the colour of the text around it).
SEARCH_ICON = ('<svg class="icon-search" viewBox="0 0 16 16" aria-hidden="true">'
               '<circle cx="6.5" cy="6.5" r="4.75" fill="none" stroke="currentColor" stroke-width="1.5"/>'
               '<path d="M10 10l4.5 4.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg>')


def nav(current, works):
    """The views as one list: Search | the themes | All published remarks | the major works."""
    def link(href, label, key, title=None):
        cur = ' aria-current="page"' if key == current else ""
        tip = f' title="{html.escape(title)}"' if title else ""
        return f'<a class="switch" href="{href}"{cur}{tip}>{html.escape(label)}</a>'
    search = link("/map/", "Search", "search").replace(">Search<", f">{SEARCH_ICON}Search<")
    links = ([search] + [link(f"/map/themes/{k}/", lab, k) for k, lab, _, _ in THEMES]
             + [link("/map/published/", "All published remarks", "published")]
             + [link(f"/map/works/{c.lower()}/", c, c, t) for c, t in works])
    # The bar stays with the item before it (no-break space), so a line never starts with one.
    return '<p class="view-switcher map-views">' + "\u00a0| ".join(links) + "</p>"


def page(title, current, works, summary, extra, blocks, params=""):
    """A view: the list of views, the search and the switchers (site/layouts/partials/
    search.html, put in place of #map-search by map.html), the count of what the map shows
    (above it, like the search's count of its results), the map, then anything else."""
    summary = f'<p class="map-summary">{summary}</p>\n' if summary else ""
    return f"""---
title: "{title}"
{params}---
{nav(current, works)}
<div id="map-search"></div>
{summary}<div class="map">{blocks}</div>
{extra}"""


# Colours of map.css (light mode): blocks from CELL (0) to HIT (1), those of the document
# under the pointer from HOVER_CELL to HOVER_HIT.
CELL, HIT, HOVER_CELL, HOVER_HIT = (0xe3, 0xe3, 0xe3), (0x22, 0x22, 0x22), (0xfb, 0xcb, 0xa4), (0x7a, 0x3c, 0x10)


def mix(a, b, t):
    return "#" + "".join(f"{round(x + (y - x) * t):02x}" for x, y in zip(a, b))


def layout_preview(order, by_doc, level_of, hover, width):
    """The page's layout (map.css: columns of ROWS blocks of 3×3px in 16px lines, 14px
    labels, a group's label divided from its first document) up to one line after the
    document `hover`. Label widths are estimated: Palatino's figures are about half an em
    wide. Returns the full columns as runs, the blocks to draw, the labels, the dividers,
    and the height."""
    line, size = 16, 4
    char, bold_char, pad, space = 7.0, 7.6, 3, 4
    runs, texts, dividers = [], [], []
    cells = defaultdict(list)  # (hovered, shade class) → [(x, y)]; class 0: blocks of a partial column
    x = y = 0
    end = None
    for group, docs in order:
        for k, doc in enumerate(d for d in docs if d in by_doc):
            if end is not None and y > end:
                return runs, cells, texts, dividers, end + line
            hov = doc == hover
            sigil = short_name(doc)
            sigil_w = len(sigil) * char
            group_w = len(group) * bold_char if k == 0 else 0
            if x > 0:
                x += space
            if x + (group_w + pad + 1 if k == 0 else 0) + sigil_w + 2 * pad + size > width:
                x, y = 0, y + line
            if k == 0:
                texts.append((x, y, group, group_w, True, False))
                x += group_w + pad
                dividers.append((x, y))
                x += 1
            texts.append((x + pad, y, sigil, sigil_w, False, hov))
            x = -(-(x + sigil_w + 2 * pad) // size) * size  # snap the blocks to the grid
            n = len(by_doc[doc])
            for j in range(0, n, ROWS):
                if x + size > width:
                    x, y = 0, y + line
                m = min(ROWS, n - j)
                if m == ROWS:
                    if runs and runs[-1][1] == y and runs[-1][0] + runs[-1][2] == x and runs[-1][3] == hov:
                        runs[-1][2] += size
                    else:
                        runs.append([x, y, size, hov])
                for r in range(m):
                    lv = level_of(doc, j + r)
                    if lv or m < ROWS:
                        cells[(hov, lv)].append((x, y + r * size))
                x += size
            if hov:
                end = y + line
    return runs, cells, texts, dividers, y + line


def preview_svg(order, by_doc, level_of, hover, aspect=1.39):
    """The beginning of a map as a picture for the home page, with the document `hover`
    under the pointer: the map up to one line after it, at the width that gives the
    picture the aspect of the home page's preview (two cells of the cover grid wide, a
    cover high). Labels are set with textLength, so that they never run into the blocks
    whatever font renders them; full columns of blocks are a pattern, so that only the
    shaded blocks are drawn one by one."""
    width = min(range(560, 1201, 8),
                key=lambda w: abs(w / layout_preview(order, by_doc, level_of, hover, w)[4] - aspect))
    runs, cells, texts, dividers, height = layout_preview(order, by_doc, level_of, hover, width)
    out = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">',
           f'<defs><pattern id="b" width="4" height="4" patternUnits="userSpaceOnUse"><rect width="3" height="3" fill="{mix(CELL, CELL, 0)}"/></pattern>'
           f'<pattern id="h" width="4" height="4" patternUnits="userSpaceOnUse"><rect width="3" height="3" fill="{mix(HOVER_CELL, HOVER_CELL, 0)}"/></pattern></defs>',
           '<rect width="100%" height="100%" fill="#fff"/>']
    out += [f'<rect x="{rx}" y="{ry}" width="{rw}" height="16" fill="url(#{"h" if hov else "b"})"/>' for rx, ry, rw, hov in runs]
    for (hov, lv) in sorted(cells):
        c0, c1 = (HOVER_CELL, HOVER_HIT) if hov else (CELL, HIT)
        d = "".join(f"M{cx} {cy}h3v3h-3z" for cx, cy in cells[(hov, lv)])
        out.append(f'<path fill="{mix(c0, c1, lv / 10)}" d="{d}"/>')
    out += [f'<rect x="{dx:.1f}" y="{dy}" width="1" height="15" fill="#777"/>' for dx, dy in dividers]
    font = "'TeX Gyre Pagella','Palatino Linotype',Palatino,'Book Antiqua',serif"
    out.append(f'<g font-family="{font}" font-size="14" fill="#999">')
    for tx, ty, text, tw, bold, hov in texts:
        attrs = (' font-weight="bold"' if bold else "") + (' fill="#000"' if hov else "")
        out.append(f'<text x="{tx:.1f}" y="{ty + 12}" textLength="{tw:.1f}" lengthAdjust="spacingAndGlyphs"{attrs}>'
                   f"{html.escape(text)}</text>")
    out.append("</g>")
    return "\n".join(out + ["</svg>"]) + "\n"


def main():
    md_dir, similar_tsv, content_dir = sys.argv[1:4]
    out_dir = os.path.join(content_dir, "map")
    os.makedirs(os.path.join(out_dir, "themes"), exist_ok=True)
    os.makedirs(os.path.join(out_dir, "works"), exist_ok=True)

    names, by_doc = read_remarks(md_dir)
    works, orders, info = read_index(md_dir)
    flat = [(d, i) for d in names for i in range(len(by_doc[d]))]
    gindex = {(d, i): g for g, (d, i) in enumerate(flat)}
    index = {by_doc[d][i]["siglum"]: g for g, (d, i) in enumerate(flat)}
    edges = load_edges(similar_tsv, index)
    total = len(flat)
    n_docs = len(names)
    rem = lambda g: by_doc[flat[g][0]][flat[g][1]]
    def summary(hit):
        """"N remarks in x of y documents.", counting the highlighted remarks (worded like the
        search's count, site/static/js/search.js)."""
        n = sum(1 for d, i in flat if hit(d, i))
        docs = len({d for d, i in flat if hit(d, i)})
        return f'{n:,} remark{"" if n == 1 else "s"} in {docs} of {n_docs} documents.'

    # Published remarks and their versions.
    published = {g for g in range(total) if rem(g)["work"]}
    val, src = spread(published, edges)

    def published_cell(d, i):
        g = gindex[(d, i)]
        r, v = by_doc[d][i], val.get(g, 0)
        here = page_ref(r["siglum"])
        if r["work"]:
            return level(v, 2), f"{here} · published in {r['work']}"
        if v > 0:
            s = rem(src[g])
            return level(v, 2), f"{here} · version of {page_ref(s['siglum'])} ({s['work']})"
        return 0, here

    # Search (the default view): every remark, nothing highlighted until a search marks its
    # hits (js/search.js).
    params = f"remarks: {total}\ndocuments: {n_docs}\nsearch: true\n"
    with open(os.path.join(out_dir, "_index.html"), "w", encoding="utf-8") as f:
        f.write(page("Nachlass Map", "search", works, "", "",
                     render_map(names, by_doc, orders["date"], info,
                                lambda d, i: (0, page_ref(by_doc[d][i]["siglum"]))), params))

    with open(os.path.join(out_dir, "published.html"), "w", encoding="utf-8") as f:
        f.write(page("Nachlass Map: All published remarks", "published", works,
                     summary(lambda d, i: gindex[(d, i)] in val), "",
                     render_map(names, by_doc, orders["date"], info, published_cell)))


    # Major works and the versions of their remarks.
    for code, title in works:
        seeds = {g for g in range(total) if rem(g)["work"] == code}
        wval, wsrc = spread(seeds, edges)

        def work_cell(d, i, wval=wval, wsrc=wsrc, seeds=seeds, code=code):
            g = gindex[(d, i)]
            here = page_ref(by_doc[d][i]["siglum"])
            if g in seeds:
                return 10, f"{here} · in {code}"
            if g in wval:
                return level(wval[g], 2), f"{here} · version of {page_ref(rem(wsrc[g])['siglum'])} ({code})"
            return 0, here

        with open(os.path.join(out_dir, "works", code.lower() + ".html"), "w", encoding="utf-8") as f:
            f.write(page(f"Nachlass Map: {code}", code, works, summary(lambda d, i, w=wval: gindex[(d, i)] in w), "",
                         render_map(names, by_doc, orders["date"], info, work_cell)))

    # Themes.
    words = set().union(*(r["words"] for d in names for r in by_doc[d]))
    lwords = {w.lower() for w in words}
    for key, label, kinds, vocab in THEMES:
        terms = [(lab, compile_term(rx, words, lwords)) for _, lab, rx in vocab]
        found, shade = {}, {}
        for d in names:
            own = []
            for i, r in enumerate(by_doc[d]):
                # What the remark contains, as written, in reading order: the notation, the
                # word forms and phrases of the terms (a formula, which has no text, as
                # "[formula]"); at most six, without repetitions.
                matches = sorted(m for _, t in terms for m in t(r))
                hits = [] if r["notation"] not in kinds else [r["expression"] or "[formula]"]
                seen = {h.lower() for h in hits}
                for _, text in matches:
                    if text.lower() not in seen and not any(text.lower() in h.lower() for h in hits):
                        seen.add(text.lower())
                        hits.append(text)
                found[(d, i)] = hits[:6] + (["…"] if len(hits) > 6 else [])
                own.append(1 if hits else 0)
            for i, s in enumerate(shades(own)):
                shade[(d, i)] = s

        def theme_cell(d, i, found=found, shade=shade):
            here = page_ref(by_doc[d][i]["siglum"])
            hits = found[(d, i)]
            return level(shade[(d, i)], 0), f"{here} · {', '.join(hits)}" if hits else here

        # Everything that counts as evidence, as one list (labels without their "…").
        included = [NOTATION_NAMES[k] for k in kinds] + [re.sub(r",? …$", "", lab) for _, lab, _ in vocab]
        extra = (f'<details class="map-terms"><summary>{len(vocab)} terms{" and formal notation" if kinds else ""}'
                 f'</summary><p>{html.escape(", ".join(included))}</p></details>\n')
        if key == PREVIEW[0]:  # the home page's preview
            with open(os.path.join(out_dir, "preview.svg"), "w", encoding="utf-8") as f:
                f.write(preview_svg(orders["date"], by_doc, lambda d, i, sh=shade: level(sh[(d, i)], 0), PREVIEW[1]))
        with open(os.path.join(out_dir, "themes", key + ".html"), "w", encoding="utf-8") as f:
            f.write(page(f"Nachlass Map: {label}", key, works, summary(lambda d, i, fd=found: fd[(d, i)]), extra,
                         render_map(names, by_doc, orders["date"], info, theme_cell)))

    # Data for the optional scripts (js/map.js, and js/search.js, which shows search results
    # as a map): the orderings, the blocks' heading ids and page references (without the
    # document's name), and the documents' descriptions.
    data = {"orders": orders,
            "anchors": {d: [r["fragment"] or "" for r in by_doc[d]] for d in names},
            "refs": {d: [page_ref(r["siglum"]).split(",", 1)[-1] for r in by_doc[d]] for d in names},
            "info": {d: f"{d} {info.get(d, '')}".strip() for d in names}}
    with open(os.path.join(out_dir, "data.json"), "w", encoding="utf-8") as f:
        json.dump(data, f, ensure_ascii=False, separators=(",", ":"))
    print(f"Nachlass Map: {total} remarks, 1 + {len(works)} works + {len(THEMES)} themes pages.")


if __name__ == "__main__":
    main()
