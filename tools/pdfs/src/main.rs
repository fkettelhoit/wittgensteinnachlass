mod prepare;
mod template;

use clap::Parser;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[derive(Parser)]
#[command(
    name = "pdfs-nachlass",
    about = "Generate PDF files from Nachlass markdown"
)]
struct Cli {
    /// Input directory containing markdown files
    #[arg(long, default_value = "../../md")]
    input: PathBuf,

    /// Output directory for PDF files
    #[arg(long, default_value = "../../pdf")]
    output: PathBuf,

    /// Directory containing SVG cover images (from covers tool)
    #[arg(long, default_value = "../../covers")]
    covers: PathBuf,

    /// Generate PDF for a single file (e.g., W-PI.md, Ms-101.md)
    #[arg(long)]
    file: Option<String>,

    /// Generate PDFs for all files
    #[arg(long)]
    all: bool,

    /// Path to pandoc binary
    #[arg(long, default_value = "pandoc")]
    pandoc: String,

    /// Font directory containing TeX Gyre Pagella OTF files
    #[arg(long, default_value = "../../site/fonts/tex-gyre")]
    font_dir: PathBuf,

    /// Font directory containing SangBleu Empire WOFF2 files (for headings)
    #[arg(long, default_value = "../../site/fonts/sangbleu")]
    heading_font_dir: PathBuf,

    /// Author name for metadata
    #[arg(long, default_value = "Ludwig Wittgenstein")]
    author: String,

    /// Path to shared transcription CSS
    #[arg(long, default_value = "../../css/transcription.css")]
    transcription_css: PathBuf,
}

fn main() {
    let cli = Cli::parse();

    let files = discover_files(&cli.input, &cli.file, cli.all);

    // Build a case-insensitive map of all .md files for resolving part references
    let file_map = build_file_map(&cli.input);

    fs::create_dir_all(&cli.output).expect("Failed to create output directory");

    // Build CSS and template once (fonts are the same for all files)
    let css = template::build_css(&cli.font_dir, &cli.heading_font_dir, &cli.transcription_css);
    let html_template = template::build_template();

    // Write template to temp file (pandoc needs it on disk)
    let tmp_template = cli.output.join("_tmp_template.html");
    fs::write(&tmp_template, &html_template).expect("Failed to write template");

    // Write CSS to temp file
    let tmp_css = cli.output.join("_tmp_style.css");
    fs::write(&tmp_css, &css).expect("Failed to write CSS");

    let mut success = 0;
    let mut failed = 0;

    for path in &files {
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        let raw = fs::read_to_string(path).expect("Failed to read markdown file");

        let prepared = if prepare::is_index_file(&raw) {
            // Multi-part work: collect all parts into one book
            let slugs = prepare::parse_index_parts(&raw);
            if slugs.is_empty() {
                eprintln!("  Skipping {} (index file with no parts)", stem);
                continue;
            }

            let mut part_raws = Vec::new();
            let mut missing = false;
            for slug in &slugs {
                if let Some(part_path) = file_map.get(&slug.to_lowercase()) {
                    let part_raw = fs::read_to_string(part_path).expect("Failed to read part file");
                    part_raws.push(part_raw);
                } else {
                    eprintln!("  WARNING: part not found for slug '{}' in {}", slug, stem);
                    missing = true;
                }
            }
            if missing && part_raws.is_empty() {
                eprintln!("  Skipping {} (no parts found)", stem);
                continue;
            }

            eprintln!(
                "  {} is a multi-part work ({} parts)",
                stem,
                part_raws.len()
            );
            prepare::prepare_book(&raw, &part_raws, &cli.author)
        } else {
            prepare::prepare(&raw, &cli.author)
        };

        if prepared.title.is_empty() {
            eprintln!("  Skipping {} (no title)", stem);
            continue;
        }

        // Warn if title is likely to overflow at 54pt on ~109mm content width
        // At 54pt SangBleu Empire, rough estimate: ~0.55em per char avg, 1em ≈ 19mm
        // Content width ≈ 109mm → ~10.4em → ~19 chars per line
        // Warn if title would need more than 3 lines
        let chars_per_line = 19;
        let max_lines = 3;
        if prepared.title.len() > chars_per_line * max_lines {
            eprintln!(
                "  WARNING: title may overflow for '{}' ({} chars, est. {} lines)",
                stem,
                prepared.title.len(),
                (prepared.title.len() + chars_per_line - 1) / chars_per_line
            );
        }

        // Write prepared markdown to temp file
        let tmp_md = cli.output.join(format!("_tmp_{}.md", stem));
        fs::write(&tmp_md, &prepared.content).expect("Failed to write temp markdown");

        // Check for cover image
        let cover_svg = cli.covers.join(format!("{}.svg", stem));
        let cover_path = if cover_svg.exists() {
            Some(fs::canonicalize(&cover_svg).unwrap_or_else(|_| cover_svg.clone()))
        } else {
            None
        };

        let pdf_path = cli.output.join(format!("{}.pdf", stem));
        let result = run_pandoc_weasyprint(
            &cli.pandoc,
            &tmp_md,
            &pdf_path,
            &tmp_template,
            &tmp_css,
            cover_path.as_ref(),
            &cli.input,
        );

        let _ = fs::remove_file(&tmp_md);

        match result {
            Ok(()) => {
                eprintln!("  {} -> {}", stem, pdf_path.display());
                success += 1;
            }
            Err(e) => {
                eprintln!("  FAILED {}: {}", stem, e);
                failed += 1;
            }
        }
    }

    // Clean up shared temp files
    let _ = fs::remove_file(&tmp_template);
    let _ = fs::remove_file(&tmp_css);

    eprintln!("\nDone: {} succeeded, {} failed", success, failed);
}

/// Build a case-insensitive map from lowercase stem to file path.
fn build_file_map(input_dir: &PathBuf) -> HashMap<String, PathBuf> {
    let mut map = HashMap::new();
    if let Ok(entries) = fs::read_dir(input_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".md") {
                let stem = name.trim_end_matches(".md").to_string();
                map.insert(stem.to_lowercase(), entry.path());
            }
        }
    }
    map
}

fn run_pandoc_weasyprint(
    pandoc: &str,
    input: &PathBuf,
    output: &PathBuf,
    template: &PathBuf,
    css: &PathBuf,
    cover: Option<&PathBuf>,
    resource_dir: &PathBuf,
) -> Result<(), String> {
    let mut cmd = Command::new(pandoc);
    cmd.arg(input);
    cmd.args(["--mathml", "--pdf-engine=weasyprint"]);
    cmd.arg(format!("--template={}", template.display()));
    // Pass CSS directly to weasyprint (not pandoc's --css, which only sets the $css$ variable)
    cmd.arg(format!("--pdf-engine-opt=-s={}", css.display()));
    // Resource path so pandoc resolves relative image paths from the markdown source dir
    cmd.arg(format!("--resource-path={}", resource_dir.display()));
    // Base URL for weasyprint to resolve relative image paths in the generated HTML
    let abs_resource_dir =
        std::fs::canonicalize(resource_dir).unwrap_or_else(|_| resource_dir.clone());
    cmd.arg(format!(
        "--pdf-engine-opt=--base-url=file://{}",
        abs_resource_dir.display()
    ));

    if let Some(cover_path) = cover {
        // Pass cover image path as a pandoc variable
        cmd.arg(format!("--variable=cover-image:{}", cover_path.display()));
    }

    cmd.arg("-o");
    cmd.arg(output);

    let result = cmd
        .output()
        .map_err(|e| format!("Failed to run pandoc: {}", e))?;

    if result.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&result.stderr);
        Err(format!("pandoc/weasyprint failed: {}", stderr.trim()))
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
