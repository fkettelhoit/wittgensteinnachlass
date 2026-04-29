mod parse;
mod svg;

use base64::Engine;
use clap::Parser;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "visualize-nachlass",
    about = "Generate SVG visualizations of work-source correspondences"
)]
struct Cli {
    /// Input directory containing markdown files
    #[arg(long, default_value = "../../md")]
    input: PathBuf,

    /// Output directory for SVG files
    #[arg(long, default_value = "../../viz")]
    output: PathBuf,

    /// Generate SVG for a single work file (e.g., W-OC.md)
    #[arg(long)]
    work: Option<String>,

    /// Generate SVGs for all work files
    #[arg(long)]
    all: bool,

    /// Path to TeX Gyre Pagella Regular OTF for embedding
    #[arg(
        long,
        default_value = "../../../site/fonts/tex-gyre/texgyrepagella-regular.otf"
    )]
    font: PathBuf,
}

fn generate_svg(
    work_path: &std::path::Path,
    input_dir: &std::path::Path,
    font_base64: &str,
) -> String {
    let work = parse::parse_work(work_path);
    let doc_order = parse::source_doc_order(&work);

    // Load all referenced source documents
    let mut source_docs: HashMap<String, parse::SourceDoc> = HashMap::new();
    for doc_name in &doc_order {
        let doc_filename = format!("{}.md", doc_name);
        let doc_path = input_dir.join(&doc_filename);
        if doc_path.exists() {
            source_docs.insert(doc_name.clone(), parse::parse_source_doc(&doc_path));
        } else {
            eprintln!("  Warning: source doc {} not found", doc_filename);
        }
    }

    let correspondences = parse::build_correspondence(&work, &source_docs);
    eprintln!(
        "  {} work remarks, {} source docs, {} correspondences",
        work.remarks.len(),
        source_docs.len(),
        correspondences.len()
    );

    svg::render(
        &work,
        &doc_order,
        &source_docs,
        &correspondences,
        font_base64,
    )
}

fn main() {
    let cli = Cli::parse();

    if !cli.input.is_dir() {
        eprintln!("Input directory does not exist: {}", cli.input.display());
        std::process::exit(1);
    }

    fs::create_dir_all(&cli.output).expect("Failed to create output directory");

    // Load and base64-encode the font
    let font_base64 = if cli.font.exists() {
        let font_bytes = fs::read(&cli.font).expect("Failed to read font file");
        base64::engine::general_purpose::STANDARD.encode(&font_bytes)
    } else {
        eprintln!(
            "Warning: font not found at {}, SVGs will use fallback fonts",
            cli.font.display()
        );
        String::new()
    };

    let work_files: Vec<PathBuf> = if let Some(ref name) = cli.work {
        let path = cli.input.join(name);
        if !path.exists() {
            eprintln!("Work file not found: {}", path.display());
            std::process::exit(1);
        }
        vec![path]
    } else if cli.all {
        let mut files: Vec<PathBuf> = fs::read_dir(&cli.input)
            .expect("Failed to read input directory")
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                name.starts_with("W-") && name.ends_with(".md")
            })
            .map(|e| e.path())
            .collect();
        files.sort();
        files
    } else {
        eprintln!("Specify --work <filename> or --all");
        std::process::exit(1);
    };

    for work_path in &work_files {
        let stem = work_path.file_stem().unwrap().to_string_lossy().to_string();

        // Skip works with no remarks (parent entries like W-RFM that only have subparts)
        let work = parse::parse_work(work_path);
        if work.remarks.is_empty() {
            eprintln!("Skipping {} (no remarks)", stem);
            continue;
        }

        eprintln!("Generating {}...", stem);
        let svg_content = generate_svg(work_path, &cli.input, &font_base64);

        let out_path = cli.output.join(format!("{}.svg", stem));
        fs::write(&out_path, &svg_content).expect("Failed to write SVG");
        eprintln!("  -> {}", out_path.display());
    }

    eprintln!("Done.");
}