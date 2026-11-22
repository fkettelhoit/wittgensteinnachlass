# visualize

Generates SVG diagrams showing how each published work draws from its source documents. Each diagram places the work's remarks as a solid bar on the left and the source documents on the right, with filled bezier shapes connecting corresponding remarks.

Each document with published remarks gets a diagram of the works that draw from it (`--all-docs`). Each document with versions of its remarks in other documents (pairs with a score of at least 0.7 in `similar/similar.tsv`, see [similar](../similar/)) gets one similar-remarks diagram per document with versions of at least 10 of its remarks (`--all-similar`, written to `<doc>-similar-<other>.svg`): the document on the left, the other document on the right. Since there are thousands of these, they embed `site/fonts/tex-gyre/texgyrepagella-regular-ascii.otf`, an ASCII subset of the font (`--similar-font`), generated with `python3 -m fontTools.subset texgyrepagella-regular.otf --unicodes="U+0020-007E,U+22EE" --layout-features='' --no-hinting --output-file=texgyrepagella-regular-ascii.otf`. `tools/add_versions.py` adds the matching `<details>` summary to the document pages and must agree with it on which documents are shown.

Embeds TeX Gyre Pagella into the SVG for consistent typography.

## Usage

```
cargo run -- --all
cargo run -- --work W-OC.md
cargo run -- --all-docs
cargo run -- --similar Ms-114.md
cargo run -- --all-similar
```

## Options

- `--input` -- markdown directory (default: `../../md`)
- `--output` -- output directory for SVG files (default: `../../viz`)
- `--work` -- generate a single work's visualization
- `--all` -- generate all work visualizations
- `--doc` / `--all-docs` -- generate one / all document visualizations
- `--similar` / `--all-similar` -- generate the similar-remarks visualizations of one / all documents
- `--similar-tsv` -- similar remarks (default: `../../similar/similar.tsv`)
- `--font` -- path to TeX Gyre Pagella OTF for embedding

## Dependencies

None beyond Rust/Cargo. The font is embedded into the SVG output.
