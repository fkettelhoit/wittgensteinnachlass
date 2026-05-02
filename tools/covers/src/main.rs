mod parse;
mod svg;

use base64::Engine;
use clap::Parser;
use std::fs;
use std::path::PathBuf;

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

    /// Path to SangBleu Empire Bold woff2
    #[arg(
        long,
        default_value = "../../../site/fonts/sangbleu/SangBleuEmpire-Bold-WebS.woff2"
    )]
    font_bold: PathBuf,

    /// Path to SangBleu Empire Regular woff2
    #[arg(
        long,
        default_value = "../../../site/fonts/sangbleu/SangBleuEmpire-Regular-WebS.woff2"
    )]
    font_regular: PathBuf,
}

fn main() {
    let cli = Cli::parse();

    let files = discover_files(&cli.input, &cli.file, cli.all);

    // Load and base64-encode fonts
    let font_bold_b64 = load_font_b64(&cli.font_bold);
    let font_regular_b64 = load_font_b64(&cli.font_regular);

    fs::create_dir_all(&cli.output).expect("Failed to create output directory");

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

        let total = data.paragraphs.len();
        let (svg, placed) = svg::render_cover(&data, &font_bold_b64, &font_regular_b64);

        let out_path = cli.output.join(format!("{}.svg", stem));
        fs::write(&out_path, &svg).expect("Failed to write SVG");
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

fn load_font_b64(path: &PathBuf) -> String {
    if path.exists() {
        let bytes = fs::read(path).expect("Failed to read font file");
        base64::engine::general_purpose::STANDARD.encode(&bytes)
    } else {
        eprintln!("Warning: font not found at {}", path.display());
        String::new()
    }
}
