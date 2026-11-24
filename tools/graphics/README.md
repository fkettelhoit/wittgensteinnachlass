# graphics

Processes SVG graphics from `graphics/` for use in ebooks and PDFs. The raw SVGs have an A4 bounding box (210 x 297 mm); this tool crops them to the actual drawing content and converts text to paths for consistent rendering.

The pipeline has two steps:

1. **Inject font-family** TeX Gyre Pagella (the font of the text) on all `<text>` elements. Only the parser's own `gen-*` drawings have `<text>`; the lettering of the other drawings is kept as drawn (as outlines), since re-setting it in another font can't be done faithfully in general (text along curves, rotated, split into pieces, sub- and superscripts).
2. **Crop and convert text to paths** -- Inkscape's `--export-area-drawing` removes whitespace and `--export-text-to-path` bakes in fonts. Inkscape finds Pagella through a `fonts.conf` the script generates (the system's fontconfig configuration plus the full Pagella fonts from `site/fonts/tex-gyre`, not the ASCII subset that shares their family name).

Output goes to `graphics-cropped/`.

## Usage

```
./process_graphics.sh
```

Set `JOBS=8` to control parallelism (default: 4).

## Dependencies

- [Inkscape](https://inkscape.org/) (`brew install --cask inkscape`) for SVG processing
