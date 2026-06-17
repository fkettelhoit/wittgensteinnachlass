mod parse;
mod svg;

use clap::Parser;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[derive(Parser)]
#[command(
    name = "covers-nachlass",
    about = "Generate SVG book covers for Nachlass documents and works"
)]
struct Cli {
    /// Input directory containing markdown files
    #[arg(long, default_value = "../../md")]
    input: PathBuf,

    /// Output directory for SVG cover files
    #[arg(long, default_value = "../../covers")]
    output: PathBuf,

    /// Generate cover for a single file (e.g., W-PI.md, Ms-101.md)
    #[arg(long)]
    file: Option<String>,

    /// Generate covers for all files
    #[arg(long)]
    all: bool,

    /// Path to SangBleu Empire Bold TTF
    #[arg(
        long,
        default_value = "../../../sangbleu/web files/SangBleuEmpire-Bold-WebS.ttf"
    )]
    font_bold: PathBuf,

    /// Path to SangBleu Empire Regular TTF
    #[arg(
        long,
        default_value = "../../../sangbleu/web files/SangBleuEmpire-Regular-WebS.ttf"
    )]
    font_regular: PathBuf,

    /// Don't embed an @font-face rule in the text SVG; rely on the fonts being installed
    /// in fontconfig instead. Required on headless Linux/CI, where Inkscape rejects
    /// @font-face file:// rules and must resolve "SangBleu Empire" by family name.
    #[arg(long)]
    no_font_face: bool,
}

fn main() {
    let cli = Cli::parse();

    let files = discover_files(&cli.input, &cli.file, cli.all);

    // Resolve font paths to absolute paths for SVG file:// references
    let font_bold_abs = fs::canonicalize(&cli.font_bold)
        .unwrap_or_else(|_| panic!("Bold font not found: {}", cli.font_bold.display()));
    let font_regular_abs = fs::canonicalize(&cli.font_regular)
        .unwrap_or_else(|_| panic!("Regular font not found: {}", cli.font_regular.display()));
    let font_bold_str = font_bold_abs.to_string_lossy();
    let font_regular_str = font_regular_abs.to_string_lossy();

    fs::create_dir_all(&cli.output).expect("Failed to create output directory");

    let mut failed = 0;

    for path in &files {
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        let content = fs::read_to_string(path).expect("Failed to read markdown file");
        let mut data = parse::parse_for_cover(&content);

        if data.title.is_empty() {
            eprintln!("  Skipping {} (no title)", stem);
            continue;
        }

        // For parent works with sub-parts (e.g. W-RFM has W-RFM-1, W-RFM-2, ...),
        // combine paragraphs from all parts to fill the cover.
        if data.paragraphs.len() <= 5 && stem.starts_with("W-") {
            let prefix = format!("{}-", stem);
            let mut part_files: Vec<PathBuf> = fs::read_dir(&cli.input)
                .expect("Failed to read input directory")
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    name.starts_with(&prefix) && name.ends_with(".md")
                })
                .map(|e| e.path())
                .collect();
            part_files.sort();

            if !part_files.is_empty() {
                for part_path in &part_files {
                    let part_content =
                        fs::read_to_string(part_path).expect("Failed to read part file");
                    let part_data = parse::parse_for_cover(&part_content);
                    data.paragraphs.extend(part_data.paragraphs);
                }
            }
        }

        // Generate text-only SVG, convert to paths via Inkscape, then combine with circles.
        // A failed conversion yields a cover with no/broken title — fail rather than ship it.
        let (text_paths, paths_ok) = convert_text_to_paths(&data.title, &font_bold_str, &font_regular_str, &cli.output, !cli.no_font_face);
        if !paths_ok {
            eprintln!("  FAILED {} (title-to-path conversion failed)", stem);
            failed += 1;
            continue;
        }

        let total = data.paragraphs.len();
        let (svg_final, placed) = svg::render_cover(&data, &text_paths);

        let out_path = cli.output.join(format!("{}.svg", stem));
        fs::write(&out_path, &svg_final).expect("Failed to write SVG");

        if placed < total {
            eprintln!(
                "  {} -> {} ({} paragraphs, WARNING: {} cut off)",
                stem,
                out_path.display(),
                total,
                total - placed
            );
        } else {
            eprintln!(
                "  {} -> {} ({} paragraphs)",
                stem,
                out_path.display(),
                total
            );
        }
    }

    if failed > 0 {
        eprintln!("\n{} cover(s) failed — failing the build.", failed);
        std::process::exit(1);
    }
}

/// Generate a text-only SVG, run Inkscape to convert text to paths, and return the
/// extracted path groups along with a success flag. Success requires both that Inkscape
/// exited cleanly and that at least one path group was extracted (an empty result means
/// the title would be missing from the cover).
fn convert_text_to_paths(title: &str, font_bold: &str, font_regular: &str, output_dir: &PathBuf, embed_font_face: bool) -> (String, bool) {
    let text_svg = svg::render_text_svg(title, font_bold, font_regular, embed_font_face);

    let tmp_path = output_dir.join("_text_tmp.svg");
    fs::write(&tmp_path, &text_svg).expect("Failed to write temp text SVG");

    let result = Command::new("inkscape")
        .arg(&tmp_path)
        .arg("--export-text-to-path")
        .arg("--export-plain-svg")
        .arg(format!("--export-filename={}", tmp_path.display()))
        .output()
        .expect("Failed to run inkscape -- is it installed?");

    let mut ok = true;
    if !result.status.success() {
        // Surface Inkscape's own diagnostics — on headless CI this is usually a missing
        // display or font, which is otherwise invisible.
        eprintln!(
            "  inkscape text-to-path conversion failed (exited {}): {}",
            result.status,
            String::from_utf8_lossy(&result.stderr).trim()
        );
        ok = false;
    }

    let inkscape_output = fs::read_to_string(&tmp_path).unwrap_or_default();
    let _ = fs::remove_file(&tmp_path);

    let paths = extract_text_paths(&inkscape_output);
    if paths.trim().is_empty() {
        eprintln!("  inkscape produced no title paths");
        ok = false;
    }

    (paths, ok)
}

/// Extract <g aria-label="..."><path .../></g> blocks from Inkscape's SVG output.
/// Inkscape formats elements across multiple lines, so we use position-based parsing.
fn extract_text_paths(svg: &str) -> String {
    let mut result = String::new();
    let mut search_from = 0;

    while let Some(pos) = svg[search_from..].find("aria-label=") {
        let abs_pos = search_from + pos;

        // Check if this aria-label is on a <g> (not a <path>)
        let tag_start = match svg[..abs_pos].rfind('<') {
            Some(i) => i,
            None => { search_from = abs_pos + 1; continue; }
        };
        if !svg[tag_start..abs_pos].contains("<g") {
            search_from = abs_pos + 1;
            continue;
        }

        // Extract the label value
        let label_start = abs_pos + "aria-label=\"".len();
        let label_end = label_start + svg[label_start..].find('"').unwrap_or(0);
        let label = &svg[label_start..label_end];

        // Find the closing </g> for this group
        let group_end = match svg[label_end..].find("</g>") {
            Some(i) => label_end + i,
            None => { search_from = abs_pos + 1; continue; }
        };
        let group_content = &svg[label_end..group_end];

        // Find the <path> element within this group, then extract d="..." and style="..."
        if let Some(path_start) = group_content.find("<path") {
            let path_region = &group_content[path_start..];
            // Search for " d=" (space-prefixed) to avoid matching "id="
            if let Some(d_pos) = path_region.find(" d=\"") {
                let d_start = d_pos + 4; // skip ' d="'
                let d_end = d_start + path_region[d_start..].find('"').unwrap_or(0);
                let d_value = &path_region[d_start..d_end];

                // Extract fill from style attribute if present
                let fill = path_region.find("style=\"").and_then(|style_pos| {
                    let s_start = style_pos + 7;
                    let s_end = s_start + path_region[s_start..].find('"')?;
                    let style = &path_region[s_start..s_end];
                    style.split(';')
                        .find_map(|part| part.trim().strip_prefix("fill:").map(|v| v.trim().to_string()))
                });

                result.push_str(&format!(r#"<g aria-label="{label}"><path d="{d_value}""#));
                if let Some(f) = fill {
                    result.push_str(&format!(r#" fill="{f}""#));
                }
                result.push_str("/></g>\n");
            }
        }

        search_from = group_end + 4;
    }
    result
}

fn discover_files(input: &PathBuf, file: &Option<String>, all: bool) -> Vec<PathBuf> {
    if let Some(name) = file {
        let path = input.join(name);
        if !path.exists() {
            eprintln!("File not found: {}", path.display());
            std::process::exit(1);
        }
        return vec![path];
    }

    if !all {
        eprintln!("Specify --file <name> or --all");
        std::process::exit(1);
    }

    let mut files: Vec<PathBuf> = fs::read_dir(input)
        .expect("Failed to read input directory")
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.ends_with(".md") && name != "index.md"
        })
        .map(|e| e.path())
        .collect();

    files.sort();
    if files.is_empty() {
        eprintln!("No markdown files found in {}", input.display());
        std::process::exit(1);
    }
    files
}
