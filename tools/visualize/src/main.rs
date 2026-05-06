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

    /// Generate SVG for a single document file (e.g., Ms-167.md)
    #[arg(long)]
    doc: Option<String>,

    /// Generate SVGs for all work files
    #[arg(long)]
    all: bool,

    /// Generate SVGs for all published document files
    #[arg(long)]
    all_docs: bool,

    /// Path to TeX Gyre Pagella Regular OTF for embedding
    #[arg(
        long,
        default_value = "../../../site/fonts/tex-gyre/texgyrepagella-regular.otf"
    )]
    font: PathBuf,
}

fn generate_work_svg(
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

/// Build a map from lowercased work filename stem to actual filename stem.
fn build_work_slug_map(input_dir: &std::path::Path) -> HashMap<String, String> {
    let mut map = HashMap::new();
    if let Ok(entries) = fs::read_dir(input_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("W-") && name.ends_with(".md") {
                let stem = name.strip_suffix(".md").unwrap().to_string();
                let slug = stem.to_lowercase();
                map.insert(slug, stem);
            }
        }
    }
    map
}

fn generate_doc_svg(
    doc_path: &std::path::Path,
    input_dir: &std::path::Path,
    work_slug_map: &HashMap<String, String>,
    font_base64: &str,
) -> Option<String> {
    let doc_stem = doc_path.file_stem().unwrap().to_string_lossy().to_string();

    // Parse the document
    let doc = parse::parse_source_doc(doc_path);

    // Find which works reference this document
    let slugs = parse::parse_doc_work_slugs(doc_path);
    if slugs.is_empty() {
        return None;
    }

    // Resolve slugs to actual work filenames and parse works
    let mut works: HashMap<String, parse::Work> = HashMap::new();
    let mut work_order: Vec<String> = Vec::new();
    for slug in &slugs {
        if let Some(stem) = work_slug_map.get(slug) {
            let work_path = input_dir.join(format!("{stem}.md"));
            if work_path.exists() {
                let work = parse::parse_work(&work_path);
                if !work.remarks.is_empty() {
                    work_order.push(stem.clone());
                    works.insert(stem.clone(), work);
                }
            }
        } else {
            eprintln!("  Warning: no work file for slug {slug}");
        }
    }

    if works.is_empty() {
        return None;
    }

    // Build correspondences
    let correspondences = parse::build_doc_correspondence(&doc_stem, &doc, &works);
    // Per-work correspondence counts
    let mut work_corr_counts: HashMap<String, usize> = HashMap::new();
    for c in &correspondences {
        *work_corr_counts.entry(c.doc_name.clone()).or_default() += 1;
    }
    let counts_str: Vec<String> = work_corr_counts
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect();
    eprintln!(
        "  {} doc remarks, {} works, {} correspondences ({})",
        doc.anchors.len(),
        works.len(),
        correspondences.len(),
        counts_str.join(", ")
    );

    // Create SourceDoc representation and title map for each work
    let mut work_docs: HashMap<String, parse::SourceDoc> = HashMap::new();
    let mut work_titles: HashMap<String, String> = HashMap::new();
    for (name, work) in &works {
        work_titles.insert(name.clone(), work.title.clone());
        work_docs.insert(name.clone(), parse::work_as_source_doc(work));
    }

    Some(svg::render_doc(
        &doc_stem,
        &doc,
        &work_order,
        &work_titles,
        &work_docs,
        &correspondences,
        font_base64,
    ))
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

    let has_work_task = cli.work.is_some() || cli.all;
    let has_doc_task = cli.doc.is_some() || cli.all_docs;

    if !has_work_task && !has_doc_task {
        eprintln!("Specify --work <filename>, --doc <filename>, --all, or --all-docs");
        std::process::exit(1);
    }

    // Generate work visualizations
    if has_work_task {
        let work_files: Vec<PathBuf> = if let Some(ref name) = cli.work {
            let path = cli.input.join(name);
            if !path.exists() {
                eprintln!("Work file not found: {}", path.display());
                std::process::exit(1);
            }
            vec![path]
        } else {
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
            let svg_content = generate_work_svg(work_path, &cli.input, &font_base64);

            let out_path = cli.output.join(format!("{}.svg", stem));
            fs::write(&out_path, &svg_content).expect("Failed to write SVG");
            eprintln!("  -> {}", out_path.display());
        }
    }

    // Generate document visualizations
    if has_doc_task {
        let work_slug_map = build_work_slug_map(&cli.input);

        let doc_files: Vec<PathBuf> = if let Some(ref name) = cli.doc {
            let path = cli.input.join(name);
            if !path.exists() {
                eprintln!("Document file not found: {}", path.display());
                std::process::exit(1);
            }
            vec![path]
        } else {
            let mut files: Vec<PathBuf> = fs::read_dir(&cli.input)
                .expect("Failed to read input directory")
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    !name.starts_with("W-") && name.ends_with(".md") && name != "index.md"
                })
                .map(|e| e.path())
                .collect();
            files.sort();
            files
        };

        for doc_path in &doc_files {
            let stem = doc_path.file_stem().unwrap().to_string_lossy().to_string();
            eprintln!("Generating doc viz {stem}...");

            match generate_doc_svg(doc_path, &cli.input, &work_slug_map, &font_base64) {
                Some(svg_content) => {
                    let out_path = cli.output.join(format!("{stem}.svg"));
                    fs::write(&out_path, &svg_content).expect("Failed to write SVG");
                    eprintln!("  -> {}", out_path.display());
                }
                None => {
                    eprintln!("  Skipping {stem} (no published remarks)");
                }
            }
        }
    }

    eprintln!("Done.");
}
