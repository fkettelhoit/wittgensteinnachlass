# covers

Generates SVG book covers for Nachlass documents and published works. Each cover features the title and a grid of circles derived from the document's paragraph content, using embedded SangBleu Empire fonts.

Multi-part works (e.g. W-RFM-1 through W-RFM-7) combine paragraphs from all parts into a single cover at the base name.

## Usage

```
cargo run -- --all
cargo run -- --file W-PI.md
```

## Options

- `--input` -- markdown directory (default: `../../md`)
- `--output` -- output directory for SVG covers (default: `../../covers`)
- `--file` -- generate a single cover
- `--all` -- generate covers for all files
- `--font-bold`, `--font-regular` -- paths to SangBleu Empire WOFF2 fonts

## Dependencies

None beyond Rust/Cargo. Fonts are embedded into the SVG output.
