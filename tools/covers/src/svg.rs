use crate::parse::{CoverData, Paragraph};
use std::collections::HashSet;

const WIDTH: f64 = 1600.0;
const HEIGHT: f64 = 2400.0;
const CELL: f64 = 32.0;
const COLS: usize = (WIDTH / CELL) as usize;
const ROWS: usize = (HEIGHT / CELL) as usize;

const TITLE_FONT_SIZE: f64 = 132.0;
const TITLE_ROW: usize = 28; // 920px / 33 ≈ 28
const TITLE_COL: usize = 3;
const TITLE_LINE_STEP: usize = 4; // 4×35=140px per word line

const BRANDING_FONT_SIZE: f64 = TITLE_FONT_SIZE;
const BRANDING_ROW: usize = 7; // 240px / 35 ≈ 7

// Colors from site stylesheet (:root variables)
const COLOR_HEADING: &str = "#000"; // --color-heading
const COLOR_TEXT: &str = "#444"; // --color-text
const COLOR_MARGIN: &str = "#bbb"; // --color-margin

/// The intended fill colour for a text group, identified by its `aria-label`. Some Inkscape
/// versions (e.g. 1.1 on CI) drop the source `fill` during text-to-path, so the covers tool
/// re-applies it from here. The branding lines (see `render_text_svg`) use the margin colour;
/// the title uses the heading colour.
pub fn text_group_fill(aria_label: &str) -> &'static str {
    if aria_label == "Writings" || aria_label.starts_with("Wittgenstein") {
        COLOR_MARGIN
    } else {
        COLOR_HEADING
    }
}

const CIRCLE_FILL: &str = "#fbcba4"; // color for filled circles
const BORDER_CIRCLE_R: f64 = 4.0;
const CIRCLE_STROKE_WIDTH: f64 = 6.0;

/// Approximate width of a character in SangBleu Empire Bold at title size.
const CHAR_WIDTH_RATIO: f64 = 0.57;

/// Generate a minimal SVG containing only the text elements and font references.
/// This SVG is meant to be processed by Inkscape's --export-text-to-path.
pub fn render_text_svg(
    title: &str,
    font_bold_path: &str,
    font_regular_path: &str,
    embed_font_face: bool,
) -> String {
    let clean_title = title
        .replace(" \u{2013} ", " ")
        .replace("\u{2013}", "");
    let words: Vec<&str> = split_title(&clean_title);

    let mut svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{WIDTH}" height="{HEIGHT}" viewBox="0 0 {WIDTH} {HEIGHT}">
"#
    );

    // Embed the fonts via @font-face for renderers that honour `src: url(file://…)`
    // (e.g. macOS Inkscape). On headless Linux, Inkscape rejects this rule ("font face
    // rule limited support") and resolving by family name via fontconfig is required —
    // and a failing @font-face for the same family shadows the fontconfig font, so it
    // must be omitted there (the fonts are registered with fontconfig instead).
    if embed_font_face {
        svg.push_str(&format!(
            r#"<style>
  @font-face {{
    font-family: "SangBleu Empire";
    src: url("file://{font_bold_path}") format("truetype");
    font-weight: 700;
    font-style: normal;
  }}
  @font-face {{
    font-family: "SangBleu Empire";
    src: url("file://{font_regular_path}") format("truetype");
    font-weight: 400;
    font-style: normal;
  }}
</style>
"#
        ));
    }

    // Title text
    let x = TITLE_COL as f64 * CELL;
    let mut y = TITLE_ROW as f64 * CELL + TITLE_FONT_SIZE * 0.85;
    for word in &words {
        let escaped = xml_escape(word);
        svg.push_str(&format!(
            r#"<g aria-label="{escaped}"><text x="{x}" y="{y:.0}" font-family="SangBleu Empire, TeX Gyre Pagella, serif" font-size="{TITLE_FONT_SIZE}" font-weight="700" fill="{COLOR_HEADING}">{escaped}</text></g>
"#,
        ));
        y += TITLE_LINE_STEP as f64 * CELL;
    }

    // Branding text
    let brand_x = TITLE_COL as f64 * CELL;
    let brand_y = BRANDING_ROW as f64 * CELL + BRANDING_FONT_SIZE * 0.85;
    svg.push_str(&format!(
        r#"<g aria-label="Wittgenstein&#x2019;s"><text x="{brand_x}" y="{brand_y:.0}" font-family="SangBleu Empire, TeX Gyre Pagella, serif" font-size="{BRANDING_FONT_SIZE}" font-weight="400" fill="{COLOR_MARGIN}">Wittgenstein&#x2019;s</text></g>
"#
    ));
    let brand_y2 = brand_y + BRANDING_FONT_SIZE * 1.1;
    svg.push_str(&format!(
        r#"<g aria-label="Writings"><text x="{brand_x}" y="{brand_y2:.0}" font-family="SangBleu Empire, TeX Gyre Pagella, serif" font-size="{BRANDING_FONT_SIZE}" font-weight="400" fill="{COLOR_MARGIN}">Writings</text></g>
"#
    ));

    svg.push_str("</svg>\n");
    svg
}

/// Returns (svg_string, paragraphs_placed).
/// `text_paths` contains pre-rendered <g aria-label="..."><path .../></g> groups
/// extracted from Inkscape's text-to-path output.
pub fn render_cover(data: &CoverData, text_paths: &str) -> (String, usize) {
    let mut svg_bg = String::new(); // halos (behind everything)
    let mut svg_fg = String::new(); // circles

    // Strip en dashes from title (used to separate parts, redundant with line breaks)
    let clean_title = data
        .title
        .replace(" \u{2013} ", " ")
        .replace("\u{2013}", "");

    // Split title into words
    let words: Vec<&str> = split_title(&clean_title);

    // Build title cell occupation map (with fuzzy margin)
    let title_cells = compute_title_cells(&words);

    // Branding area
    let branding_cells = compute_branding_cells(data.subtitle.is_some());

    // Walk grid left-to-right, top-to-bottom. For each position that isn't
    // excluded (title, branding), place the next paragraph's circle.
    // This preserves reading order — the cover is a sequential visualization
    // of the remarks in the text. Empty positions get border-style circles.
    let mut para_iter = data.paragraphs.iter().peekable();
    let mut placed = 0;

    let mut skipped = 0;
    for row in 0..ROWS {
        for col in 0..COLS {
            if title_cells.contains(&(row, col)) || branding_cells.contains(&(row, col)) {
                skipped += 1;
            }
        }
    }

    let total_lines = (data.paragraphs.len() + skipped) / COLS;
    let start_row = ROWS.saturating_sub(total_lines) / 2;

    for row in 0..ROWS {
        for col in 0..COLS {
            if title_cells.contains(&(row, col)) || branding_cells.contains(&(row, col)) {
                continue;
            }

            let cx = col as f64 * CELL + CELL / 2.0;
            let cy = row as f64 * CELL + CELL / 2.0;

            if row >= start_row {
                if let Some(para) = para_iter.next() {
                    let r = circle_radius(para.len);
                    let (class, has_halo) = circle_class(para);

                    if has_halo {
                        let halo_r = r + 5.0;
                        svg_bg.push_str(&format!(
                            r#"<circle cx="{cx}" cy="{cy}" r="{halo_r:.1}" class="h"/>"#
                        ));
                        svg_bg.push('\n');
                    }

                    svg_fg.push_str(&format!(
                        r#"<circle cx="{cx}" cy="{cy}" r="{r:.1}" class="{class}"/>"#
                    ));
                    svg_fg.push('\n');
                    placed += 1;
                    continue;
                }
            }
            // Fill remaining positions with border-style circles
            svg_fg.push_str(&format!(
                    r#"<circle cx="{cx}" cy="{cy}" r="{BORDER_CIRCLE_R}" class="b"/>"#
                ));
            svg_fg.push('\n');
        }
    }

    // Assemble SVG
    let mut svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{WIDTH}" height="{HEIGHT}" viewBox="0 0 {WIDTH} {HEIGHT}">
<style>
.h{{fill:{CIRCLE_FILL};stroke:{CIRCLE_FILL};stroke-width:6}}
.f{{fill:{CIRCLE_FILL};stroke:{COLOR_TEXT};stroke-width:6}}
.s{{fill:{COLOR_TEXT};stroke:{COLOR_TEXT};stroke-width:6}}
.o{{fill:none;stroke:{COLOR_TEXT};stroke-width:{CIRCLE_STROKE_WIDTH}}}
.b{{fill:none;stroke:{COLOR_TEXT};stroke-width:2}}
</style>
<rect width="100%" height="100%" fill="white"/>
"#
    );

    svg.push_str(&svg_bg);
    svg.push_str(&svg_fg);
    svg.push_str(text_paths);
    svg.push_str("</svg>\n");
    (svg, placed)
}

/// Circle radius based on paragraph length, capped to stay within cell.
fn circle_radius(len: usize) -> f64 {
    let r = (len as f64 + 1.0).ln().powi(2) / 4.0;
    r.min(CELL * 0.42) // cap at ~17px
}

/// Split title into words for display (one word per line).
/// Splits on spaces. For long words with hyphens, splits after the hyphen.
/// Short document IDs (Ms-101, Ts-213) are kept as single words.
fn split_title(title: &str) -> Vec<&str> {
    let space_words: Vec<&str> = title.split_whitespace().collect();
    let mut words = Vec::new();
    for w in space_words {
        // Only split on hyphens if the word is long (>10 chars)
        // to avoid splitting document IDs like "Ms-101", "Ts-201a1"
        if w.len() > 10 && w.contains('-') {
            let mut start = 0;
            for (i, c) in w.char_indices() {
                if c == '-' && i + 1 < w.len() {
                    words.push(&w[start..=i]);
                    start = i + 1;
                }
            }
            if start < w.len() {
                words.push(&w[start..]);
            }
        } else {
            words.push(w);
        }
    }
    words
}

/// Compute which grid cells the title text occupies (with margins).
fn compute_title_cells(words: &[&str]) -> HashSet<(usize, usize)> {
    let mut cells = HashSet::new();
    for (i, word) in words.iter().enumerate() {
        let row = TITLE_ROW + i * TITLE_LINE_STEP;
        let cols_per_char = (CHAR_WIDTH_RATIO * TITLE_FONT_SIZE) / CELL;
        let word_cols = word
            .chars()
            .map(|c| match c {
                'i' | 'l' | '-' | '1' => 0.5,
                'T' => 1.0,
                'm' | 'w' => 1.2,
                c if c.is_uppercase() => 1.2,
                _ => 1.0,
            })
            .map(|w| w * cols_per_char)
            .sum::<f64>()
            .ceil() as usize
            + 1;

        // Mark cells with 1-cell margin around each word line
        for r in row.saturating_sub(1)..=(row + TITLE_LINE_STEP).min(ROWS - 1) {
            for c in TITLE_COL.saturating_sub(1)..=(TITLE_COL + word_cols + 1).min(COLS - 1) {
                cells.insert((r, c));
            }
        }
    }
    cells
}

/// Compute cells reserved for the branding text in the top-left.
/// Each line gets its own width, with a margin row below "Writings".
fn compute_branding_cells(_has_subtitle: bool) -> HashSet<(usize, usize)> {
    let mut cells = HashSet::new();
    let line1_end = 30 + TITLE_COL;
    let line2_end = 19 + TITLE_COL;

    let start_col = TITLE_COL.saturating_sub(1);
    let r0 = BRANDING_ROW; // first line starts here

    // "Wittgenstein\u2019s" text + bottom margin (one extra row above for top padding)
    for r in (r0.saturating_sub(1))..(r0 + 5) {
        for c in start_col..line1_end.min(COLS) {
            cells.insert((r, c));
        }
    }
    // "Writings" text (starts ~4 rows after first line)
    let r1 = r0 + 4;
    for r in r1..(r1 + 6) {
        for c in start_col..line2_end.min(COLS) {
            cells.insert((r, c));
        }
    }
    cells
}

/// Determine circle CSS class based on paragraph content.
/// Returns (class_name, has_halo).
fn circle_class(para: &Paragraph) -> (&'static str, bool) {
    if para.question_marks <= para.periods && para.periods > 0 {
        ("f", true) // filled with halo
    } else if para.has_bold {
        ("s", false) // solid dark
    } else {
        ("o", false) // hollow/open
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
