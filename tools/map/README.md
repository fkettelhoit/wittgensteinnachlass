# map

Generates the **Nachlass Map** at `/map/`: every remark of the Nachlass (all `md/Ms-*.md` and `md/Ts-*.md`) as a small block, so that one can see at a glance where in the Nachlass something occurs. Run by `tools/build_site.sh`:

```
python3 map.py ../../md ../../similar/similar.tsv ../../site/content
```

## Output

`site/content/map/`, as static Hugo pages (layout: `site/layouts/partials/map.html`, styles: `site/static/css/map.css`):

- `_index.html`: the search (`/map/`, the default view): every remark, nothing highlighted until a search marks its hits,
- `published.html`: the published remarks (`/map/published/`),
- `themes/<key>.html`: one page per theme (`/map/themes/<key>/`), see `themes.py`,
- `works/<code>.html`: one page per major work, i.e. those whose covers the home page shows directly (`/map/works/<code>/`),
- `data.json`: the orderings, the remarks' heading ids and page references, and the documents' descriptions, for the optional scripts (`map.js`, and `search.js`, which can show search results as a map),
- `preview.svg`: the beginning of the Mathematics map (by date), for the home page.

The pages work without JavaScript: the blocks are in date order, the ordering of the home page's "By date" list, and each block's description shows in a popover when the pointer rests on it (`data-t`, drawn by CSS), e.g. `Ms-130,1[1] · version of Ts-227a,289[3] (PI)`. With JavaScript, the search and the line of switchers (`site/layouts/partials/search.html`, put in place of `#map-search` by `map.html`; `site/static/js/search.js`) add the search on the Search view (marking its hits on the map, or as a list), the orderings by name and by format and the bars layout ("As bars": one document per line, as long as its number of remarks) on every view; `site/static/js/map.js` links each block to its remark on the document page and switches between the views in place, keeping the search.

## Layout

Each remark is a block of 3×3 px (plus a 1 px gap). Blocks fill columns of four, top to bottom, so that each line of the map is as high as a line of the 14 px labels, and the browser wraps the lines like text. Each document is a run of blocks after its number (without Ms-/Ts-); a group's label (a year; with JavaScript also Manuscripts/Typescripts or a format) precedes its first document, divided from it like the document and page columns of the document pages.

## What is highlighted

**Published:** remarks that appear in an edited work (their heading links to it), plus their versions: remarks connected to a published remark through similar pairs (`similar/similar.tsv`) at or above the site-wide minimum score (`similar/min-score.txt`), i.e. the pairs that the document pages link as versions. A version's value is the best product of the pair scores along such a chain, so a remark 0.7 similar to a published one counts 0.7 and one 0.5 similar to that 0.35. The shade shows the value.

**Works:** the same, starting from one work's remarks.

**Themes:** a remark has evidence for a theme if it contains one of the theme's terms (`themes.py`) or, for Mathematics and Logic, formal notation (LaTeX formulas and plain-text arithmetic, logical symbols). Its shade is ½ · its own evidence (0 or 1) + ½ · the share of the 5 remarks on either side (in the same document) with evidence, so a remark without a term inside a stretch about the subject is shaded too.

## Choice of themes

The themes are defined structurally, without using the editors' selections, so that the map can be compared with them. A theme is kept if

1. it is a family of terms, not a single word (that would be better served by a search),
2. its terms occur near each other in the manuscripts (within a few remarks), i.e. it is one investigation,
3. it occupies different stretches of the manuscripts than the other themes (otherwise the two are merged; e.g. sensation, emotion, meaning-experience and pretence form one region, Psychology, whereas aspect-seeing is a region of its own; logic and mathematics are separate regions),
4. it is defined by a subject, not by a register or an editorial collection (e.g. not "Culture & Art"),
5. it has a few hundred remarks or more.

A term is included if its ordinary meaning is clearly about the subject; words with an everyday second meaning (Satz, Regel, weiß, gewiß, Gefühl, …) are left out. Each theme also has English terms for the documents written or dictated in English.
