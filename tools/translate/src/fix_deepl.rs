use crate::common::*;
use crate::verify;
use regex::Regex;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

pub struct FixDeeplArgs {
    pub input: PathBuf,
    pub translated: PathBuf,
    pub deepl_key: String,
    pub glossary: Option<PathBuf>,
    pub verbose: bool,
    pub emphasis_tolerance: usize,
}

/// Critical issue checks that should prevent accepting a DeepL translation.
/// Emphasis mismatches are non-critical (DeepL can't fix them, but the translation is still good).
fn has_critical_issues(issues: &[verify::Issue]) -> bool {
    issues
        .iter()
        .any(|i| matches!(i.check, "structure" | "length" | "untranslated"))
}

pub fn run(args: &FixDeeplArgs) {
    let client = reqwest::blocking::Client::new();

    // Set up glossary
    let glossary = match &args.glossary {
        Some(path) => {
            let content = fs::read_to_string(path).expect("Failed to read glossary");
            Glossary::parse(&content)
        }
        None => Glossary::empty(),
    };

    let glossary_id: Option<String> = if glossary.has_terms() {
        match setup_deepl_glossary(&client, &args.deepl_key, &glossary) {
            Ok(id) => Some(id),
            Err(e) => {
                eprintln!("Warning: failed to create DeepL glossary: {}", e);
                None
            }
        }
    } else {
        None
    };

    // Build context for ambiguous terms
    let ambiguous_ctx = deepl_ambiguous_context(&glossary);
    if !ambiguous_ctx.is_empty() {
        eprintln!(
            "Ambiguous terms will be passed as context ({} chars)",
            ambiguous_ctx.len()
        );
    }

    // Run verify
    let verify_args = verify::VerifyArgs {
        input: args.input.clone(),
        translated: args.translated.clone(),
        emphasis_tolerance: args.emphasis_tolerance,
    };
    let all_issues = verify::run(&verify_args);

    // Filter to fixable issues in completed .md files
    let fixable: Vec<&verify::Issue> = all_issues
        .iter()
        .filter(|i| {
            let path = args.translated.join(&i.file);
            path.exists() && !i.remark_id.is_empty()
        })
        .collect();

    if fixable.is_empty() {
        eprintln!("\nNo issues to fix.");
    } else {
        // Deduplicate: one re-translation per (file, remark_id)
        let mut fix_targets: Vec<(String, String)> = fixable
            .iter()
            .map(|i| (i.file.clone(), i.remark_id.clone()))
            .collect();
        fix_targets.sort();
        fix_targets.dedup();

        // Read existing tracking file to skip already-attempted remarks
        let tracking_path = args.translated.join("deepl-remarks.md");
        let already_attempted: std::collections::HashSet<String> =
            if let Ok(content) = fs::read_to_string(&tracking_path) {
                Regex::new(r"^## (.+)$")
                    .unwrap()
                    .captures_iter(&content)
                    .map(|c| c[1].to_string())
                    .collect()
            } else {
                std::collections::HashSet::new()
            };

        // Filter out already-attempted remarks
        let before = fix_targets.len();
        fix_targets.retain(|(f, r)| !already_attempted.contains(&format!("{}:{}", f, r)));
        if fix_targets.len() < before {
            eprintln!(
                "\nSkipping {} already-attempted remark(s) (logged in deepl-remarks.md)",
                before - fix_targets.len()
            );
        }

        if fix_targets.is_empty() {
            eprintln!("No new remarks to fix.");
        }

        if !fix_targets.is_empty() {
        eprintln!("\nRe-translating {} remark(s) via DeepL...", fix_targets.len());

        // Open the tracking file for appending
        let mut tracking_file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&tracking_path)
            .expect("Failed to open deepl-remarks.md");

        // Write header if file is new/empty
        if fs::metadata(&tracking_path).map(|m| m.len()).unwrap_or(0) == 0 {
            writeln!(tracking_file, "# Remarks translated via DeepL\n")
                .expect("Failed to write header");
        }

        for (file_name, remark_id) in &fix_targets {
            let de_path = args.input.join(file_name);
            let en_path = args.translated.join(file_name);

            let de_content = fs::read_to_string(&de_path).expect("Failed to read German file");
            let en_content = fs::read_to_string(&en_path).expect("Failed to read English file");

            let (_, de_remarks) = parse_document(&de_content);
            let (en_preamble, mut en_remarks) = parse_document(&en_content);

            // Find the remark index by matching remark_id
            let idx = de_remarks
                .iter()
                .position(|r| anchor_from_doc_heading(&r.heading) == *remark_id);
            let Some(idx) = idx else {
                eprintln!("  {}:{} — skipping (remark not found)", file_name, remark_id);
                continue;
            };
            if idx >= en_remarks.len() {
                eprintln!("  {}:{} — skipping (out of bounds)", file_name, remark_id);
                continue;
            }

            // Collect issue descriptions for this remark
            let issue_desc: Vec<String> = all_issues
                .iter()
                .filter(|i| &i.file == file_name && &i.remark_id == remark_id)
                .map(|i| format!("{}: {}", i.check, i.description))
                .collect();

            eprint!("  {}:{}...", file_name, remark_id);

            let de_body = &de_remarks[idx].body;
            let (text_with_math_placeholders, math_blocks) = extract_math(de_body);
            // Convert emphasis to HTML for DeepL (it preserves HTML natively)
            let html_text = emphasis_to_html(&text_with_math_placeholders);

            // Context: ambiguous glossary terms + preceding remark's translation
            let mut context_parts = Vec::new();
            if !ambiguous_ctx.is_empty() {
                context_parts.push(ambiguous_ctx.clone());
            }
            if idx > 0 {
                context_parts.push(en_remarks[idx - 1].body.clone());
            }
            let context = if context_parts.is_empty() {
                None
            } else {
                Some(context_parts.join("\n\n"))
            };

            let old_body = en_remarks[idx].body.clone();
            let mut best_body = old_body.clone();
            let mut succeeded = false;
            let mut remaining_issues = Vec::new();

            if args.verbose {
                eprintln!("\n--- DEEPL REQUEST ---\n{}\n---", html_text);
            }

            match call_deepl(
                &client,
                &args.deepl_key,
                &html_text,
                glossary_id.as_deref(),
                context.as_deref(),
            ) {
                Ok(translated) => {
                    if args.verbose {
                        eprintln!("--- DEEPL RESPONSE ---\n{}\n---", translated);
                    }
                    let restored_em = fix_emphasis_markers(&emphasis_from_html(&translated), de_body);
                    let restored = restore_math(&restored_em, &math_blocks);
                    best_body = restored;

                    // Verify — accept unless there are critical issues
                    let en_remark = Remark {
                        heading: en_remarks[idx].heading.clone(),
                        body: best_body.clone(),
                    };
                    let mut issues = Vec::new();
                    verify::verify_remark(
                        file_name, idx, remark_id, &de_remarks[idx], &en_remark, &mut issues,
                        args.emphasis_tolerance,
                    );

                    if issues.is_empty() {
                        succeeded = true;
                    } else if !has_critical_issues(&issues) {
                        // Non-critical issues (emphasis) — accept anyway
                        succeeded = true;
                        remaining_issues = issues;
                    } else {
                        remaining_issues = issues;
                    }
                }
                Err(e) => {
                    let msg = e.to_string();
                    eprintln!(" DeepL ERROR: {}", msg);
                    if msg.contains("Quota exceeded") {
                        eprintln!("\nDeepL quota exceeded — stopping. Re-run next month or upgrade your plan.");
                        // Clean up glossary before exiting
                        if let Some(gid) = &glossary_id {
                            delete_deepl_glossary(&client, &args.deepl_key, gid);
                        }
                        return;
                    }
                }
            }

            if succeeded {
                // Only replace if DeepL's translation passes verification
                en_remarks[idx] = Remark {
                    heading: en_remarks[idx].heading.clone(),
                    body: best_body.clone(),
                };

                let mut output = en_preamble.clone();
                for r in &en_remarks {
                    output.push_str(&format!("\n{}\n\n{}\n", r.heading, r.body));
                }
                fs::write(&en_path, &output).expect("Failed to write fixed file");
                if remaining_issues.is_empty() {
                    eprintln!(" done (DeepL)");
                } else {
                    let notes: Vec<String> = remaining_issues
                        .iter()
                        .map(|i| format!("{}: {}", i.check, i.description))
                        .collect();
                    eprintln!(" done (DeepL, minor: {})", notes.join("; "));
                }
            } else {
                // Keep the old translation — critical structural issues
                eprintln!(" skipped (DeepL: critical issues)");
            }

            // Log to tracking file (including old translation so nothing is lost)
            writeln!(tracking_file, "## {}:{}\n", file_name, remark_id)
                .expect("Failed to write tracking");
            writeln!(
                tracking_file,
                "**Issue:** {}\n",
                issue_desc.join("; ")
            )
            .expect("Failed to write tracking");
            let status = if !succeeded {
                "unchanged (critical issues)".to_string()
            } else if remaining_issues.is_empty() {
                "fixed".to_string()
            } else {
                let notes: Vec<String> = remaining_issues
                    .iter()
                    .map(|i| format!("{}: {}", i.check, i.description))
                    .collect();
                format!("fixed (minor issues: {})", notes.join("; "))
            };
            writeln!(tracking_file, "**Status:** {}\n", status)
                .expect("Failed to write tracking");
            writeln!(tracking_file, "**German:**\n\n{}\n", de_body)
                .expect("Failed to write tracking");
            writeln!(tracking_file, "**Old translation:**\n\n{}\n", old_body)
                .expect("Failed to write tracking");
            writeln!(
                tracking_file,
                "**DeepL translation{}:**\n\n{}\n",
                if succeeded { "" } else { " (not used — failed verification)" },
                best_body
            )
            .expect("Failed to write tracking");
            writeln!(tracking_file, "---\n").expect("Failed to write tracking");
        }
        } // if !fix_targets.is_empty()
    }

    // Reassemble works
    eprintln!("\nReassembling work files...");
    let url_map = build_remark_url_map(&args.translated, &args.input);

    let mut work_files: Vec<_> = fs::read_dir(&args.input)
        .expect("Failed to read input dir")
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.starts_with("W-") && name.ends_with(".md")
        })
        .map(|e| e.path())
        .collect();
    work_files.sort();

    for work_path in &work_files {
        let filename = work_path.file_name().unwrap().to_string_lossy();
        let out_path = args.translated.join(&*filename);
        eprint!("  {}...", filename);
        let missing = assemble_work(work_path, &url_map, &out_path);
        if missing > 0 {
            eprintln!(" done ({} remarks missing)", missing);
        } else {
            eprintln!(" done");
        }
    }

    // Clean up glossary
    if let Some(gid) = &glossary_id {
        delete_deepl_glossary(&client, &args.deepl_key, gid);
        eprintln!("Cleaned up DeepL glossary.");
    }

    eprintln!("\nDone.");
}
