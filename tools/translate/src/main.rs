mod check;
mod common;
mod fix_deepl;
mod translate;
mod verify;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

fn load_dotenv() {
    // Silently load .env if present; ignore if missing
    let _ = dotenvy::dotenv();
}

/// Resolve glossary: error if not provided and not explicitly disabled.
fn resolve_glossary(glossary: Option<PathBuf>, no_glossary: bool) -> Option<PathBuf> {
    if no_glossary {
        eprintln!("Glossary explicitly disabled.");
        return None;
    }
    let path = glossary.unwrap_or_else(|| PathBuf::from("glossary.md"));
    if !path.exists() {
        eprintln!(
            "Error: glossary not found at '{}'. Provide --glossary <path> or pass --no-glossary to proceed without one.",
            path.display()
        );
        std::process::exit(1);
    }
    eprintln!("Using glossary: {}", path.display());
    Some(path)
}

#[derive(Parser)]
#[command(
    name = "translate-nachlass",
    about = "Translate Wittgenstein Nachlass markdown files"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Translate document files and assemble works (using ollama)
    Translate {
        /// Input directory containing German markdown files
        #[arg(long, default_value = "../../md")]
        input: PathBuf,

        /// Output directory for translated files
        #[arg(long, default_value = "../../md-en")]
        output: PathBuf,

        /// Ollama model name
        #[arg(long, default_value = "translategemma:27b")]
        model: String,

        /// Ollama API base URL
        #[arg(long, default_value = "http://localhost:11434")]
        ollama_url: String,

        /// Path to a glossary file (defaults to glossary.md)
        #[arg(long)]
        glossary: Option<PathBuf>,

        /// Proceed without a glossary
        #[arg(long)]
        no_glossary: bool,

        /// Log prompts, German text, and translations to stderr
        #[arg(long)]
        verbose: bool,

        /// Skip verification and fixing of already-translated documents (still detects changed remarks)
        #[arg(long)]
        no_verify: bool,

        /// Only apply mechanical fixes (prefix/suffix changes); skip LLM translation
        #[arg(long)]
        auto_fix_only: bool,

        /// Ollama context window size in tokens (default 8192)
        #[arg(long, default_value_t = 8192)]
        num_ctx: usize,

        /// Fraction of budget for history vs. new remarks: 0.0 = all new, 1.0 = all history (default 0.5)
        #[arg(long, default_value_t = 0.5)]
        context_ratio: f64,

        /// Allowed emphasis mismatch (number of underscores/asterisks, default 4)
        #[arg(long, default_value_t = 4)]
        emphasis_tolerance: usize,
    },

    /// Verify translation quality (docs only, includes partial files)
    Verify {
        /// Input directory containing German markdown files
        #[arg(long, default_value = "../../md")]
        input: PathBuf,

        /// Directory containing translated English markdown files
        #[arg(long, default_value = "../../md-en")]
        translated: PathBuf,

        /// Allowed emphasis mismatch (number of underscores/asterisks, default 4)
        #[arg(long, default_value_t = 4)]
        emphasis_tolerance: usize,
    },

    /// Fix broken remarks using the DeepL API
    FixDeepl {
        /// Input directory containing German markdown files
        #[arg(long, default_value = "../../md")]
        input: PathBuf,

        /// Directory containing translated English markdown files
        #[arg(long, default_value = "../../md-en")]
        translated: PathBuf,

        /// DeepL API key (or set DEEPL_API_KEY env var)
        #[arg(long, env = "DEEPL_API_KEY")]
        deepl_key: String,

        /// Path to a glossary file (defaults to glossary.md)
        #[arg(long)]
        glossary: Option<PathBuf>,

        /// Proceed without a glossary
        #[arg(long)]
        no_glossary: bool,

        /// Log prompts and responses to stderr
        #[arg(long)]
        verbose: bool,

        /// Allowed emphasis mismatch (number of underscores/asterisks, default 4)
        #[arg(long, default_value_t = 4)]
        emphasis_tolerance: usize,
    },

    /// Migrate headings from German originals to English translations
    Migrate {
        /// Input directory containing German markdown files
        #[arg(long, default_value = "../../md")]
        input: PathBuf,

        /// Directory containing translated English markdown files
        #[arg(long, default_value = "../../md-en")]
        translated: PathBuf,
    },

    /// Build-broken gate: fail if English translations are missing, stale, or fail
    /// quality verification (read-only, no LLM). Requires full git history.
    Check {
        /// Input directory containing German markdown files
        #[arg(long, default_value = "../../md")]
        input: PathBuf,

        /// Directory containing translated English markdown files
        #[arg(long, default_value = "../../md-en")]
        translated: PathBuf,

        /// Allowed emphasis mismatch (number of underscores/asterisks, default 4)
        #[arg(long, default_value_t = 4)]
        emphasis_tolerance: usize,
    },
}

fn main() {
    load_dotenv();
    let cli = Cli::parse();

    match cli.command {
        Command::Translate {
            input,
            output,
            model,
            ollama_url,
            glossary,
            no_glossary,
            verbose,
            no_verify,
            auto_fix_only,
            num_ctx,
            context_ratio,
            emphasis_tolerance,
        } => {
            translate::run(&translate::TranslateArgs {
                input,
                output,
                model,
                ollama_url,
                glossary: resolve_glossary(glossary, no_glossary),
                verbose,
                no_verify,
                auto_fix_only,
                num_ctx,
                context_ratio,
                emphasis_tolerance,
            });
        }
        Command::Verify { input, translated, emphasis_tolerance } => {
            let issues = verify::run(&verify::VerifyArgs { input, translated, emphasis_tolerance });
            if !issues.is_empty() {
                std::process::exit(1);
            }
        }
        Command::FixDeepl {
            input,
            translated,
            deepl_key,
            glossary,
            no_glossary,
            verbose,
            emphasis_tolerance,
        } => {
            fix_deepl::run(&fix_deepl::FixDeeplArgs {
                input,
                translated,
                deepl_key,
                glossary: resolve_glossary(glossary, no_glossary),
                verbose,
                emphasis_tolerance,
            });
        }
        Command::Migrate { input, translated } => {
            migrate_headings(&input, &translated);
        }
        Command::Check { input, translated, emphasis_tolerance } => {
            check::run(&check::CheckArgs { input, translated, emphasis_tolerance });
        }
    }
}

fn migrate_headings(input: &std::path::Path, translated: &std::path::Path) {
    use common::*;

    let mut files_updated = 0;
    let mut headings_updated = 0;

    let entries: Vec<_> = std::fs::read_dir(translated)
        .expect("Failed to read translated dir")
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.ends_with(".md") && !name.starts_with("W-") && name != "index.md" && name != "deepl-remarks.md"
        })
        .collect();

    for entry in &entries {
        let name = entry.file_name().to_string_lossy().to_string();
        let de_path = input.join(&name);
        let en_path = entry.path();

        if !de_path.exists() {
            continue;
        }

        let de_content = std::fs::read_to_string(&de_path).expect("Failed to read German file");
        let en_content = std::fs::read_to_string(&en_path).expect("Failed to read English file");

        let (de_preamble, de_remarks) = parse_document(&de_content);
        let (_, en_remarks) = parse_document(&en_content);

        if de_remarks.len() != en_remarks.len() {
            eprintln!(
                "  {} — skipping (remark count mismatch: DE {} vs EN {})",
                name, de_remarks.len(), en_remarks.len()
            );
            continue;
        }

        let mut changed = false;
        let mut new_remarks: Vec<Remark> = Vec::new();

        for (de, en) in de_remarks.iter().zip(en_remarks.iter()) {
            let de_anchor = anchor_from_doc_heading(&de.heading);
            let en_anchor = anchor_from_doc_heading(&en.heading);

            if dedup_anchor(&de_anchor) == dedup_anchor(&en_anchor) {
                if de.heading != en.heading {
                    new_remarks.push(Remark {
                        heading: de.heading.clone(),
                        body: en.body.clone(),
                    });
                    headings_updated += 1;
                    changed = true;
                } else {
                    new_remarks.push(Remark {
                        heading: en.heading.clone(),
                        body: en.body.clone(),
                    });
                }
            } else {
                eprintln!(
                    "  {} — warning: anchor mismatch at DE '{}' vs EN '{}', skipping remark",
                    name, de_anchor, en_anchor
                );
                new_remarks.push(Remark {
                    heading: en.heading.clone(),
                    body: en.body.clone(),
                });
            }
        }

        if changed {
            let mut output = de_preamble;
            for r in &new_remarks {
                output.push_str(&format!("\n{}\n\n{}\n", r.heading, r.body));
            }
            std::fs::write(&en_path, &output).expect("Failed to write updated file");
            files_updated += 1;
            eprintln!("  {} — updated", name);
        }
    }

    eprintln!(
        "\nDone. Updated {} heading(s) across {} file(s).",
        headings_updated, files_updated
    );
}
