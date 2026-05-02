# graphics

Processes SVG graphics from `graphics/` for use in ebooks and PDFs. The raw SVGs have an A4 bounding box (210 x 297 mm); this tool crops them to the actual drawing content and converts text to paths for consistent rendering.

The pipeline has three steps:

1. **Convert text-as-path to `<text>`** -- `convert_text_paths.py` uses Inkscape's `--query-all` to get bounding boxes for aria-labeled path elements and replaces them with proper `<text>` elements.
2. **Inject font-family** on all `<text>` elements.
3. **Crop and convert text to paths** -- Inkscape's `--export-area-drawing` removes whitespace and `--export-text-to-path` bakes in fonts.

Output goes to `graphics-cropped/`.

## Usage

```
./process_graphics.sh
```

Set `JOBS=8` to control parallelism (default: 4).

## Dependencies

- [Inkscape](https://inkscape.org/) (`brew install --cask inkscape`) for SVG processing
- Python 3 for `convert_text_paths.py`
