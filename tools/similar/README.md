# similar

Finds textually similar remarks across all manuscripts and typescripts (`md/Ms-*.md`, `md/Ts-*.md`) and writes them to `similar/similar.tsv`. Needs `md/dates.tsv` from the parser. Most matches are other versions of the same remark: drafts, revisions, typescript copies.

```
cargo run --release                         # write ../../similar/similar.tsv
cargo run --release -- --samples 5          # print example pairs per score band instead
cargo run --release -- --samples 5 --samples-max-shared 5   # only pairs with little overlap
```

## Method

1. **Remarks** are split at `### ` headings. Each is identified by its Bergen-style siglum (`Ms-115,118[2]et119[1]`), read from all facsimile labels in the heading, linked or not. The transcription's own extensions (`-relocated`, `.2`) are kept. The parser guarantees that sigla are unique within a document; the tool fails if one occurs twice. `tools/add_versions.py` and `tools/visualize` read sigla from headings the same way, so their keys match those in `similar.tsv`.
2. **Normalization:** section titles, date lines, rules, math, images and series numbers are dropped. Deletions (`<s>`) are dropped unless they make up more than half of the remark, since Wittgenstein often struck whole remarks after copying them. The text is lowercased, `ß` → `ss`, `&`/`and` → `und`, dotted abbreviations are joined (`d.h.` → `dh`, `u.s.w.` → `usw`), hyphens inside words are removed, and punctuation is ignored. No stemming, no stop words.
3. **Shingles:** each remark becomes the set of its overlapping 3-word sequences (`--k`). Shingles that occur in more than 100 remarks (`--max-df`) are stock phrases and are ignored.
4. **Score:** IDF-weighted containment: the IDF weight of the shared shingles divided by the total IDF weight of the shorter remark. A short remark fully contained in a longer one therefore scores 1.0.
5. **Filter:** pairs need at least 3 shared shingles (`--min-shared`) and a score of at least 0.4 (`--min-score`). Each pair must also be among the 20 best matches of at least one of its remarks (`--top`).

## Output

`similar/similar.tsv` is generated, not committed: `make content` and `make viz` depend on `make similar`, so every build (including CI) regenerates it from the current `md/`. It is tab-separated with a header row. Each pair appears once, ordered by document and position. The output is deterministic.

| column | meaning |
|---|---|
| `a`, `b` | sigla of the two remarks |
| `score` | IDF-weighted containment, 0.40–1.00 |
| `shared` | number of shared shingles |
| `order` | chronology: `<` if `a` is earlier than `b`, `>` if later, `~` if undetermined |

The chronology comes from the per-remark dates in Bergen's XML, which the parser writes to `md/dates.tsv`. Within a document, position decides. Across documents, disjoint date ranges are ordered, and so are overlapping ranges where one both starts and ends no later than the other. Identical or nested ranges stay `~`.

Pairs with high scores are reliably versions of each other. Below roughly 0.7, pairs where a very short remark shares only 3–5 shingles with another remark are sometimes just a shared stock phrase. Consumers can filter on `score` and `shared`.

## Display

The minimum score of the pairs shown anywhere on the site is set in one place, `similar/min-score.txt` (currently 0.7), which `tools/add_versions.py` and `tools/visualize` read from next to `similar.tsv`. `similar.tsv` itself keeps all pairs down to 0.4, so the cutoff can be changed without rerunning this tool.

`tools/add_versions.py` (run by `tools/build_site.sh`) turns pairs at or above that score into links to the published documents (those linked from `md/all.md`) in the margin of the German document pages, below the facsimile links. Each remark shows the first entry of its chronology (`←` earlier, `≈` concurrent, `→` later version, labelled with the document number without its Ms-/Ts- prefix), followed by `+N`, which opens the rest.

Documents with versions of their remarks in other documents (same threshold) also get a collapsible summary below the document's own one, listing the other documents by the number of the document's remarks with versions in them. Those with at least 10 get a visualization of where in them these versions are (`tools/visualize --all-similar`), the rest follow on one line.
