use crate::common::*;
use crate::verify::{load_ignore_words, verify_remark, Issue};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct CheckArgs {
    pub input: PathBuf,
    pub translated: PathBuf,
    pub emphasis_tolerance: usize,
}

/// Read-only build-broken gate (no LLM). Reports English translations that are
/// missing, stale (the German source changed since the translation was last
/// committed), or fail quality verification, and exits non-zero if any are found.
///
/// Staleness is determined from git history exactly like `translate`: for each
/// translated doc we find the commit that last touched `md-en/<doc>`, read the German
/// source as of that commit, and flag any remark whose German body has since changed.
/// This requires the full git history (e.g. `actions/checkout` with `fetch-depth: 0`).
pub fn run(args: &CheckArgs) {
    if !args.input.is_dir() {
        eprintln!("Input directory does not exist: {}", args.input.display());
        std::process::exit(1);
    }
    if !args.translated.is_dir() {
        eprintln!(
            "Translated directory does not exist: {}",
            args.translated.display()
        );
        std::process::exit(1);
    }

    // Repo root and the paths of md/ and md-en/ relative to it (for git lookups).
    let repo_dir = args.input.parent().unwrap_or(Path::new("."));
    let en_rel = args.translated.strip_prefix(repo_dir).unwrap_or(&args.translated);
    let de_rel = args.input.strip_prefix(repo_dir).unwrap_or(&args.input);

    let skip_remarks = load_skip_remarks(Path::new("."));
    let ignore_words = load_ignore_words(Path::new("."));

    // Document order from all.md (works are assembled from these docs, so checking the
    // docs covers the works' content too).
    let index_path = args.input.join("all.md");
    let docs = if index_path.exists() {
        let index_content = std::fs::read_to_string(&index_path).expect("Failed to read all.md");
        parse_index_order(&index_content).0
    } else {
        eprintln!("No all.md found, scanning input directory alphabetically");
        let mut docs = Vec::new();
        for entry in std::fs::read_dir(&args.input).expect("Failed to read input dir") {
            let name = entry.unwrap().file_name().to_string_lossy().to_string();
            if name.ends_with(".md") && name != "all.md" && !name.starts_with("W-") {
                docs.push(name);
            }
        }
        docs.sort();
        docs
    };

    let mut missing: Vec<String> = Vec::new();
    let mut stale: Vec<String> = Vec::new();
    let mut issues: Vec<Issue> = Vec::new();

    for filename in &docs {
        let de_path = args.input.join(filename);
        if !de_path.exists() {
            continue;
        }
        let de_content = std::fs::read_to_string(&de_path).expect("Failed to read German file");
        let (_, de_remarks) = parse_document(&de_content);

        let en_path = args.translated.join(filename);

        // Whole file untranslated: every non-skipped remark is missing.
        if !en_path.exists() {
            for de in &de_remarks {
                let rid = anchor_from_doc_heading(&de.heading);
                if rid.is_empty() || should_skip_remark(&skip_remarks, filename, &rid) {
                    continue;
                }
                missing.push(format!("{}:{}", filename, rid));
            }
            continue;
        }

        let en_content = std::fs::read_to_string(&en_path).expect("Failed to read English file");
        let (_, en_remarks) = parse_document(&en_content);
        let en_by_anchor: HashMap<String, &Remark> = en_remarks
            .iter()
            .map(|r| (anchor_from_doc_heading(&r.heading), r))
            .collect();

        // Missing remarks + quality verification of existing pairs.
        for (i, de) in de_remarks.iter().enumerate() {
            let rid = anchor_from_doc_heading(&de.heading);
            if rid.is_empty() || should_skip_remark(&skip_remarks, filename, &rid) {
                continue;
            }
            match en_by_anchor.get(&rid) {
                None => missing.push(format!("{}:{}", filename, rid)),
                Some(en) => verify_remark(
                    filename,
                    i,
                    &rid,
                    de,
                    en,
                    &mut issues,
                    args.emphasis_tolerance,
                    &ignore_words,
                ),
            }
        }

        // Stale remarks: German changed since the English file was last committed.
        let en_file_rel = format!("{}/{}", en_rel.display(), filename);
        let de_file_rel = format!("{}/{}", de_rel.display(), filename);
        if let Some(last_commit) = git_last_commit(repo_dir, &en_file_rel) {
            if let Some(old_de_content) = git_show(repo_dir, &last_commit, &de_file_rel) {
                for (_, anchor) in detect_changed_remarks(&old_de_content, &de_content) {
                    if anchor.is_empty() || should_skip_remark(&skip_remarks, filename, &anchor) {
                        continue;
                    }
                    // Remarks with no English yet are already reported as missing; only
                    // flag as stale those that exist in English but whose German moved on.
                    if en_by_anchor.contains_key(&anchor) {
                        stale.push(format!("{}:{}", filename, anchor));
                    }
                }
            }
        }
    }

    // Report.
    let broken = !missing.is_empty() || !stale.is_empty() || !issues.is_empty();
    if !missing.is_empty() {
        eprintln!("\nMissing translations ({}):", missing.len());
        for m in &missing {
            println!("MISSING {}", m);
        }
    }
    if !stale.is_empty() {
        eprintln!("\nStale translations — German changed since last translated ({}):", stale.len());
        for s in &stale {
            println!("STALE {}", s);
        }
    }
    if !issues.is_empty() {
        eprintln!("\nQuality issues ({}):", issues.len());
        for issue in &issues {
            println!(
                "QUALITY {}:{} [{}] {}",
                issue.file, issue.remark_id, issue.check, issue.description
            );
        }
    }

    if broken {
        eprintln!(
            "\nBuild is broken: {} missing, {} stale, {} quality issue(s).",
            missing.len(),
            stale.len(),
            issues.len()
        );
        std::process::exit(1);
    }
    eprintln!("All English translations present, up to date, and verified.");
}
