# visualize

Generates SVG diagrams showing how each published work draws from its source documents. Each diagram places the work's remarks as a solid bar on the left and the source documents on the right, with filled bezier shapes connecting corresponding remarks.

Embeds TeX Gyre Pagella into the SVG for consistent typography.

## Usage

```
cargo run -- --all
cargo run -- --work W-OC.md
```

## Options

- `--input` -- markdown directory (default: `../../md`)
- `--output` -- output directory for SVG files (default: `../../viz`)
- `--work` -- generate a single work's visualization
- `--all` -- generate all visualizations
- `--font` -- path to TeX Gyre Pagella OTF for embedding

## Dependencies

None beyond Rust/Cargo. The font is embedded into the SVG output.
