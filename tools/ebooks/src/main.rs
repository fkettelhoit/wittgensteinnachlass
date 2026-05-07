mod prepare;

use clap::Parser;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[derive(Parser)]
#[command(
    name = "ebooks-nachlass",
    about = "Generate EPUB ebooks from Nachlass markdown files"
)]
struct Cli {
    /// Input directory containing markdown files
    #[arg(long, default_value = "../../md")]
    input: PathBuf,

    /// Output directory for EPUB files
    #[arg(long, default_value = "../../epub")]
    output: PathBuf,

    /// Directory containing SVG cover images (from covers tool)
    #[arg(long, default_value = "../../covers")]
    covers: PathBuf,

    /// Generate ebook for a single file (e.g., W-PI.md, Ms-101.md)
    #[arg(long)]
    file: Option<String>,

    /// Generate ebooks for all files
    #[arg(long)]
    all: bool,

    /// Path to pandoc binary
    #[arg(long, default_value = "pandoc")]
    pandoc: String,

    /// Path to rsvg-convert binary (for SVG-to-PNG cover conversion)
    #[arg(long, default_value = "rsvg-convert")]
    rsvg_convert: String,

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

    fs::create_dir_all(&cli.output).expect("Failed to create output directory");

    let mut success = 0;
    let mut failed = 0;

    for path in &files {
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();

        let raw = fs::read_to_string(path).expect("Failed to read markdown file");

        // Check if this is a parent file with parts
        let prepared = if let Some((title, parts)) = prepare::detect_parent(&raw) {
            let mut part_bodies: Vec<(String, String)> = Vec::new();
            let mut all_found = true;
            for part in &parts {
                if let Some(filename) = prepare::slug_to_filename(&part.slug, &cli.input) {
                    let part_path = cli.input.join(&filename);
                    let part_raw = fs::read_to_string(&part_path)
                        .unwrap_or_else(|_| panic!("Failed to read part file: {}", part_path.display()));
                    let body = prepare::prepare_body(&part_raw);
                    part_bodies.push((part.chapter_name.clone(), body));
                } else {
                    eprintln!("  Warning: part file not found for slug '{}' in {}", part.slug, stem);
                    all_found = false;
                }
            }
            if !all_found || part_bodies.is_empty() {
                eprintln!("  Skipping {} (missing parts)", stem);
                continue;
            }
            prepare::prepare_merged(&title, &part_bodies, &cli.author)
        } else {
            prepare::prepare(&raw, &cli.author)
        };

        if prepared.title.is_empty() {
            eprintln!("  Skipping {} (no title)", stem);
            continue;
        }

        // Write prepared markdown to temp file
        let tmp_md = cli.output.join(format!("_tmp_{}.md", stem));
        fs::write(&tmp_md, &prepared.content).expect("Failed to write temp markdown");

        // Convert cover SVG to PNG if available
        let cover_svg = cli.covers.join(format!("{}.svg", stem));
        let cover_png = cli.output.join(format!("_tmp_{}_cover.png", stem));
        let has_cover = if cover_svg.exists() {
            convert_svg_to_png(&cli.rsvg_convert, &cover_svg, &cover_png)
        } else {
            false
        };

        // Build pandoc command
        let epub_path = cli.output.join(format!("{}.epub", stem));
        let result = run_pandoc(
            &cli.pandoc,
            &tmp_md,
            &epub_path,
            if has_cover { Some(&cover_png) } else { None },
            &cli.input,
            &cli.transcription_css,
        );

        // Clean up temp files
        let _ = fs::remove_file(&tmp_md);
        let _ = fs::remove_file(&cover_png);

        match result {
            Ok(()) => {
                eprintln!("  {} -> {}", stem, epub_path.display());
                success += 1;
            }
            Err(e) => {
                eprintln!("  FAILED {}: {}", stem, e);
                failed += 1;
            }
        }
    }

    eprintln!("\nDone: {} succeeded, {} failed", success, failed);
}

fn convert_svg_to_png(rsvg_convert: &str, svg: &PathBuf, png: &PathBuf) -> bool {
    let status = Command::new(rsvg_convert)
        .args([
            "-w", "1600",
            "-h", "2400",
            "-b", "#ffffff",
        ])
        .arg(svg)
        .arg("-o")
        .arg(png)
        .status();

    match status {
        Ok(s) if s.success() => true,
        Ok(s) => {
            eprintln!("  Warning: rsvg-convert exited with {}", s);
            false
        }
        Err(e) => {
            eprintln!("  Warning: rsvg-convert not available ({})", e);
            false
        }
    }
}

fn run_pandoc(
    pandoc: &str,
    input: &PathBuf,
    output: &PathBuf,
    cover: Option<&PathBuf>,
    resource_path: &PathBuf,
    css: &PathBuf,
) -> Result<(), String> {
    let mut cmd = Command::new(pandoc);
    cmd.arg(input);
    cmd.args(["--mathml", "-o"]);
    cmd.arg(output);
    cmd.arg(format!("--resource-path={}", resource_path.display()));

    if css.exists() {
        cmd.arg(format!("--css={}", css.display()));
    }

    if let Some(cover_path) = cover {
        cmd.arg(format!("--epub-cover-image={}", cover_path.display()));
    }

    let result = cmd.output().map_err(|e| format!("Failed to run pandoc: {}", e))?;

    if result.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&result.stderr);
        Err(format!("pandoc failed: {}", stderr.trim()))
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
