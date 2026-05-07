use crate::common::*;
use crate::verify;
use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

const MAX_REMARK_RETRIES: usize = 2;
const MAX_DOC_RETRIES: usize = 2;

/// Collect context entries that fit within the character budget, most recent first.
fn adaptive_context(context_window: &VecDeque<String>, max_chars: usize) -> Vec<String> {
    let mut chars = 0;
    let mut result = Vec::new();
    for body in context_window.iter().rev() {
        if chars + body.len() > max_chars {
            break;
        }
        chars += body.len();
        result.push(body.clone());
    }
    result.reverse();
    result
}

/// Translate a single remark with inline verification and retries.
/// Returns the translated body, or the original German body on failure.
#[allow(clippy::too_many_arguments)]
fn translate_remark(
    client: &reqwest::blocking::Client,
    ollama_url: &str,
    model: &str,
    system_msg: &str,
    glossary_section: &str,
    context: &[String],
    text_with_placeholders: &str,
    math_blocks: &[String],
    de_remark: &Remark,
    filename: &str,
    idx: usize,
    remark_id: &str,
    verbose: bool,
    num_ctx: usize,
    emphasis_tolerance: usize,
) -> String {
    let mut best_body = de_remark.body.clone();
    let use_em_tags = text_with_placeholders.contains('_') || text_with_placeholders.contains("**");

    for attempt in 0..=MAX_REMARK_RETRIES {
        // Strategy: attempt 0 uses <em> tags, retries use checklist
        let (user_msg, restore_html) = if attempt == 0 && use_em_tags {
            // Convert _emphasis_ to <em> tags — the model preserves HTML more reliably
            let html_text = emphasis_to_html(text_with_placeholders);
            (
                translation_user_msg(&html_text, context, glossary_section),
                true,
            )
        } else if attempt == 0 {
            (
                translation_user_msg(text_with_placeholders, context, glossary_section),
                false,
            )
        } else {
            // Retries use checklist + fix instructions (complementary strategy)
            let en_remark = Remark {
                heading: de_remark.heading.clone(),
                body: best_body.clone(),
            };
            let mut issues = Vec::new();
            verify::verify_remark(filename, idx, remark_id, de_remark, &en_remark, &mut issues, emphasis_tolerance);
            let issue_refs: Vec<&verify::Issue> = issues.iter().collect();
            let instructions = fix_instructions(&issue_refs, &de_remark.body);

            let mut msg = String::new();
            if !glossary_section.is_empty() {
                msg.push_str(
                    "Use the following glossary for established translations \
                     of key philosophical terms. Follow these conventions strictly:\n\n",
                );
                msg.push_str(glossary_section);
                msg.push_str("\n\n");
            }
            let checklist = emphasis_checklist(text_with_placeholders);
            if !checklist.is_empty() {
                msg.push_str(&checklist);
            }
            msg.push_str(&format!(
                "Translate the following German text to English.\n\n\
                 IMPORTANT — a previous translation attempt had problems. \
                 Make sure your translation avoids them:\n\n\
                 {}\n\n\
                 German text:\n\n{}",
                instructions, text_with_placeholders
            ));
            (msg, false)
        };

        if verbose {
            eprintln!(
                "\n--- SYSTEM PROMPT ---\n{}\n--- USER MSG ---\n{}\n---",
                system_msg, user_msg
            );
        }

        match call_ollama(client, ollama_url, model, system_msg, &user_msg, num_ctx) {
            Ok(translated) => {
                if verbose {
                    eprintln!("--- EN ---\n{}\n---", translated);
                }
                let restored = if restore_html {
                    emphasis_from_html(&translated)
                } else {
                    translated
                };
                let fixed = fix_emphasis_markers(&restored, &de_remark.body);
                best_body = restore_math(&fixed, math_blocks);

                let en_remark = Remark {
                    heading: de_remark.heading.clone(),
                    body: best_body.clone(),
                };
                let mut issues = Vec::new();
                verify::verify_remark(filename, idx, remark_id, de_remark, &en_remark, &mut issues, emphasis_tolerance);

                if issues.is_empty() {
                    eprintln!(" done");
                    return best_body;
                }

                // If emphasis is the only issue and DE has ≥2 emphasized segments,
                // try segment-by-segment emphasis repair before full retry
                let only_emphasis = issues.iter().all(|i| i.check == "emphasis");
                if only_emphasis && count_emphasized_segments(&de_remark.body) >= 2 {
                    if let Some(repaired) = repair_emphasis_by_segment(
                        client, ollama_url, model, num_ctx,
                        &de_remark.body, &best_body, verbose,
                    ) {
                        // Re-verify the repaired version
                        let repaired_remark = Remark {
                            heading: de_remark.heading.clone(),
                            body: repaired.clone(),
                        };
                        let mut repair_issues = Vec::new();
                        verify::verify_remark(
                            filename, idx, remark_id, de_remark,
                            &repaired_remark, &mut repair_issues, emphasis_tolerance,
                        );
                        if repair_issues.is_empty() {
                            eprintln!(" done (emphasis repaired)");
                            return repaired;
                        }
                        // Use repaired version as best even if not perfect
                        let repaired_emph_issues = repair_issues.iter()
                            .filter(|i| i.check == "emphasis").count();
                        let orig_emph_issues = issues.iter()
                            .filter(|i| i.check == "emphasis").count();
                        if repaired_emph_issues < orig_emph_issues {
                            best_body = repaired;
                        }
                    }
                }

                if attempt < MAX_REMARK_RETRIES {
                    eprint!(
                        " ({} issue(s), retry {}/{})",
                        issues.len(),
                        attempt + 1,
                        MAX_REMARK_RETRIES
                    );
                } else {
                    eprintln!(" done with {} issue(s)", issues.len());
                }
            }
            Err(e) => {
                eprintln!(" ERROR: {}", e);
                break;
            }
        }
    }
    best_body
}

/// Generate LLM-actionable fix instructions from a list of verify issues.
fn fix_instructions(issues: &[&verify::Issue], de_body: &str) -> String {
    let mut instructions = Vec::new();
    for issue in issues {
        let instruction = match issue.check {
            "structure" => {
                if issue.description.contains("Math block") {
                    let re = regex::Regex::new(r"(?s)<math[\s>].*?</math>").unwrap();
                    let blocks: Vec<&str> = re.find_iter(de_body).map(|m| m.as_str()).collect();
                    if blocks.is_empty() {
                        "Copy all <math>...</math> blocks from the original verbatim.".to_string()
                    } else {
                        format!(
                            "Your translation MUST contain these exact math blocks, \
                             copied verbatim:\n{}",
                            blocks.join("\n")
                        )
                    }
                } else if issue.description.contains("HTML tags") {
                    let no_math = regex::Regex::new(r"(?s)<math[\s>].*?</math>")
                        .unwrap()
                        .replace_all(de_body, "")
                        .into_owned();
                    let re = regex::Regex::new(r"<[^>]+>").unwrap();
                    let tags: Vec<&str> = re.find_iter(&no_math).map(|m| m.as_str()).collect();
                    format!(
                        "Your translation MUST contain these exact HTML tags, \
                         copied verbatim: {}",
                        tags.join(", ")
                    )
                } else if issue.description.contains("Image") {
                    "Copy all image references (![](path)) from the original verbatim.".to_string()
                } else {
                    issue.description.clone()
                }
            }
            "emphasis" => {
                if issue.description.contains("Underscore") {
                    let de_count = de_body.chars().filter(|&c| c == '_').count();
                    format!(
                        "The original has exactly {} underscore (_) characters for emphasis. \
                         Your translation must also have exactly {} underscores.",
                        de_count, de_count
                    )
                } else {
                    let de_count = de_body.chars().filter(|&c| c == '*').count();
                    format!(
                        "The original has exactly {} asterisk (*) characters. \
                         Your translation must also have exactly {} asterisks.",
                        de_count, de_count
                    )
                }
            }
            "quotes" => {
                "Use English curly quotes \u{201c}...\u{201d} for quotations. \
                 NEVER use straight ASCII double quotes (\")."
                    .to_string()
            }
            "length" => {
                if issue.description.contains("truncation") {
                    "Translate the ENTIRE text from beginning to end. \
                     Do not skip, summarize, or omit any sentences."
                        .to_string()
                } else {
                    "Translate only what is in the original. \
                     Do not add extra content, explanations, or commentary."
                        .to_string()
                }
            }
            "untranslated" => {
                "Translate ALL German words to English. \
                 Do not leave any German words untranslated (except proper names)."
                    .to_string()
            }
            _ => issue.description.clone(),
        };
        if !instructions.contains(&instruction) {
            instructions.push(instruction);
        }
    }
    instructions.join("\n")
}

/// Verify a translated document and re-translate broken remarks (up to MAX_DOC_RETRIES passes).
#[allow(clippy::too_many_arguments)]
fn verify_and_fix_doc(
    filename: &str,
    de_path: &Path,
    en_path: &Path,
    client: &reqwest::blocking::Client,
    ollama_url: &str,
    model: &str,
    system_msg: &str,
    glossary: &Glossary,
    verbose: bool,
    num_ctx: usize,
    emphasis_tolerance: usize,
    skip_remarks: &std::collections::HashSet<String>,
) {
    let de_content = fs::read_to_string(de_path).expect("Failed to read German file");
    let (_, de_remarks) = parse_document(&de_content);

    for doc_pass in 1..=MAX_DOC_RETRIES {
        let en_content = fs::read_to_string(en_path).expect("Failed to read translated file");
        let (en_preamble, mut en_remarks) = parse_document(&en_content);

        // Find remarks with issues
        let mut broken: Vec<usize> = Vec::new();
        for (i, (de, en)) in de_remarks.iter().zip(en_remarks.iter()).enumerate() {
            let rid = anchor_from_doc_heading(&de.heading);
            if should_skip_remark(skip_remarks, filename, &rid) {
                continue;
            }
            let mut issues = Vec::new();
            verify::verify_remark(filename, i, &rid, de, en, &mut issues, emphasis_tolerance);
            if !issues.is_empty() {
                broken.push(i);
            }
        }

        if broken.is_empty() {
            break;
        }

        eprintln!(
            "  doc pass {}/{}: re-translating {} remark(s)...",
            doc_pass, MAX_DOC_RETRIES, broken.len()
        );

        let mut changed = false;
        for &i in &broken {
            let rid = anchor_from_doc_heading(&de_remarks[i].heading);
            eprint!("    {}:{}...", filename, rid);

            let (text_with_placeholders, math_blocks) = extract_math(&de_remarks[i].body);
            let glossary_section = glossary.filter_for(&de_remarks[i].body);
            let empty_context: Vec<String> = Vec::new();

            let new_body = translate_remark(
                client,
                ollama_url,
                model,
                system_msg,
                &glossary_section,
                &empty_context,
                &text_with_placeholders,
                &math_blocks,
                &de_remarks[i],
                filename,
                i,
                &rid,
                verbose,
                num_ctx,
                emphasis_tolerance,
            );

            if new_body != en_remarks[i].body {
                en_remarks[i] = Remark {
                    heading: en_remarks[i].heading.clone(),
                    body: new_body,
                };
                changed = true;
            }
        }

        if changed {
            let mut output = en_preamble;
            for r in &en_remarks {
                output.push_str(&format!("\n{}\n\n{}\n", r.heading, r.body));
            }
            fs::write(en_path, &output).expect("Failed to write fixed file");
        } else {
            break;
        }
    }
}

pub struct TranslateArgs {
    pub input: PathBuf,
    pub output: PathBuf,
    pub model: String,
    pub ollama_url: String,
    pub glossary: Option<PathBuf>,
    pub verbose: bool,
    pub no_verify: bool,
    pub num_ctx: usize,
    pub context_ratio: f64,
    pub emphasis_tolerance: usize,
}

/// Sliding window for computing average remark translation time.
struct TimeEstimator {
    durations: VecDeque<f64>,
    max_window: usize,
    remaining: usize,
}

impl TimeEstimator {
    fn new(total_remaining: usize) -> Self {
        Self {
            durations: VecDeque::new(),
            max_window: 1000,
            remaining: total_remaining,
        }
    }

    fn record(&mut self, secs: f64) {
        self.durations.push_back(secs);
        if self.durations.len() > self.max_window {
            self.durations.pop_front();
        }
        self.remaining = self.remaining.saturating_sub(1);
    }

    fn record_n(&mut self, secs: f64, n: usize) {
        // Record per-remark average for batched translations
        let per_remark = secs / n as f64;
        for _ in 0..n {
            self.durations.push_back(per_remark);
            if self.durations.len() > self.max_window {
                self.durations.pop_front();
            }
        }
        self.remaining = self.remaining.saturating_sub(n);
    }

    fn estimate_remaining(&self) -> Option<String> {
        if self.durations.is_empty() || self.remaining == 0 {
            return None;
        }
        let avg = self.durations.iter().sum::<f64>() / self.durations.len() as f64;
        let secs = (avg * self.remaining as f64) as u64;
        Some(format_duration(secs))
    }
}

fn format_duration(total_secs: u64) -> String {
    let days = total_secs / 86400;
    let hours = (total_secs % 86400) / 3600;
    let mins = (total_secs % 3600) / 60;
    if days > 0 {
        format!("{}d {}h {}m", days, hours, mins)
    } else if hours > 0 {
        format!("{}h {}m", hours, mins)
    } else {
        format!("{}m", mins)
    }
}

/// Count total remaining remarks across untranslated doc files.
fn count_remaining_remarks(doc_files: &[String], input: &Path, output: &Path) -> usize {
    let mut total = 0;
    for filename in doc_files {
        let out_path = output.join(filename);
        if out_path.exists() {
            continue; // already done
        }
        let path = input.join(filename);
        if !path.exists() {
            continue;
        }
        let content = fs::read_to_string(&path).expect("Failed to read file");
        let (_, remarks) = parse_document(&content);

        // Subtract already-done remarks in partial files
        let tmp_path = out_path.with_extension("md.partial");
        let done = if tmp_path.exists() {
            let partial = fs::read_to_string(&tmp_path).expect("Failed to read partial");
            let (_, done_remarks) = parse_document(&partial);
            done_remarks.len()
        } else {
            0
        };

        total += remarks.len().saturating_sub(done);
    }
    total
}

pub fn run(args: &TranslateArgs) {
    if !args.input.is_dir() {
        eprintln!("Input directory does not exist: {}", args.input.display());
        std::process::exit(1);
    }
    fs::create_dir_all(&args.output).expect("Failed to create output directory");

    // Compute character budgets from num_ctx:
    // ~3 chars per token (conservative for German), reserve half for output,
    // subtract ~500 tokens for system prompt + glossary general section
    let usable_chars = args.num_ctx.saturating_sub(500) * 3 / 2;
    let max_context_chars = (usable_chars as f64 * args.context_ratio) as usize;
    let max_batch_chars = usable_chars - max_context_chars;
    eprintln!(
        "Context window: {} tokens, usable: ~{} chars ({} context / {} batch)",
        args.num_ctx, usable_chars, max_context_chars, max_batch_chars
    );

    // Load skip list from the tool directory (where the binary runs from)
    let skip_remarks = load_skip_remarks(Path::new("."));

    // Enforce skip list on existing translations: replace skip-listed remarks
    // with the German original (they should not have been translated)
    if !skip_remarks.is_empty() {
        for entry in &skip_remarks {
            if let Some((filename, remark_id)) = entry.split_once(':') {
                let de_path = args.input.join(filename);
                let en_path = args.output.join(filename);
                if !de_path.exists() || !en_path.exists() {
                    continue;
                }
                let de_content = fs::read_to_string(&de_path).expect("Failed to read DE");
                let en_content = fs::read_to_string(&en_path).expect("Failed to read EN");
                let (_, de_remarks) = parse_document(&de_content);
                let (en_preamble, mut en_remarks) = parse_document(&en_content);

                let mut changed = false;
                for (i, de_r) in de_remarks.iter().enumerate() {
                    let rid = anchor_from_doc_heading(&de_r.heading);
                    if rid == remark_id && i < en_remarks.len() && en_remarks[i].body != de_r.body {
                        en_remarks[i] = Remark {
                            heading: en_remarks[i].heading.clone(),
                            body: de_r.body.clone(),
                        };
                        changed = true;
                        eprintln!("  {}:{} — replaced with German original (skip list)", filename, remark_id);
                    }
                }
                if changed {
                    let mut output = en_preamble;
                    for r in &en_remarks {
                        output.push_str(&format!("\n{}\n\n{}\n", r.heading, r.body));
                    }
                    fs::write(&en_path, &output).expect("Failed to write");
                }
            }
        }
    }

    let glossary = match &args.glossary {
        Some(path) => {
            let content = fs::read_to_string(path).expect("Failed to read glossary");
            Glossary::parse(&content)
        }
        None => Glossary::empty(),
    };

    let system_msg = translation_system_prompt(glossary.general_section());
    let client = reqwest::blocking::Client::new();

    // Parse all.md for file ordering (docs and works)
    let index_path = args.input.join("all.md");
    let (doc_files, work_files) = if index_path.exists() {
        let index_content = fs::read_to_string(&index_path).expect("Failed to read all.md");
        parse_index_order(&index_content)
    } else {
        eprintln!("No all.md found, using alphabetical order");
        let mut docs = Vec::new();
        for entry in fs::read_dir(&args.input).expect("Failed to read input dir") {
            let entry = entry.unwrap();
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".md") && name != "all.md" && !name.starts_with("W-") {
                docs.push(name);
            }
        }
        docs.sort();
        (docs, Vec::new())
    };

    // Pre-flight: assemble works whose source docs are all already translated
    if !work_files.is_empty() {
        let mut assembled = 0;
        let url_map = build_remark_url_map(&args.output, &args.input);
        for filename in &work_files {
            let work_path = args.input.join(filename);
            if !work_path.exists() {
                continue;
            }
            let source_docs = work_source_docs(&work_path);
            let all_translated = source_docs.iter().all(|doc| {
                let path = args.output.join(doc);
                path.exists() && path.extension().map_or(false, |e| e == "md")
            });
            if all_translated && !source_docs.is_empty() {
                let out_path = args.output.join(filename);
                let missing = assemble_work(&work_path, &url_map, &out_path);
                assembled += 1;
                if missing > 0 {
                    eprintln!(
                        "  Assembled {} ({} remarks missing translations)",
                        filename, missing
                    );
                } else {
                    eprintln!("  Assembled {}", filename);
                }
            }
        }
        if assembled > 0 {
            eprintln!(
                "Pre-assembled {} work(s) from existing translations.\n",
                assembled
            );
        }
    }

    // Phase 1: Verify+fix existing translations
    if args.no_verify {
        eprintln!("Skipping verification of existing translations (--no-verify).\n");
    }
    let existing: Vec<&String> = doc_files
        .iter()
        .filter(|f| args.output.join(f).exists())
        .collect();
    if !existing.is_empty() && !args.no_verify {
        eprintln!("Verifying {} existing translation(s)...", existing.len());
        for filename in &existing {
            let de_path = args.input.join(filename);
            let en_path = args.output.join(filename);
            if !de_path.exists() {
                continue;
            }
            eprint!("  {}...", filename);
            // Quick check: any issues?
            let de_content = fs::read_to_string(&de_path).expect("Failed to read German file");
            let en_content = fs::read_to_string(&en_path).expect("Failed to read English file");
            let (_, de_remarks) = parse_document(&de_content);
            let (_, en_remarks) = parse_document(&en_content);
            let mut has_issues = false;
            for (i, (de, en)) in de_remarks.iter().zip(en_remarks.iter()).enumerate() {
                let rid = anchor_from_doc_heading(&de.heading);
                if should_skip_remark(&skip_remarks, filename, &rid) {
                    continue;
                }
                let mut issues = Vec::new();
                verify::verify_remark(filename, i, &rid, de, en, &mut issues, args.emphasis_tolerance);
                if !issues.is_empty() {
                    has_issues = true;
                    break;
                }
            }
            if has_issues {
                eprintln!(" issues found, fixing...");
                verify_and_fix_doc(
                    filename,
                    &de_path,
                    &en_path,
                    &client,
                    &args.ollama_url,
                    &args.model,
                    &system_msg,
                    &glossary,
                    args.verbose,
                    args.num_ctx,
                    args.emphasis_tolerance,
                    &skip_remarks,
                );
            } else {
                eprintln!(" ok");
            }
        }
        eprintln!();
    }

    // Detect and re-translate changed remarks in existing translations
    if !existing.is_empty() {
        // Find the repo root (parent of md/ and md-en/)
        let repo_dir = args.input.parent().unwrap_or(Path::new("."));
        let en_rel = args.output.strip_prefix(repo_dir).unwrap_or(&args.output);
        let de_rel = args.input.strip_prefix(repo_dir).unwrap_or(&args.input);

        eprintln!("Checking for changed remarks...");
        for filename in &existing {
            let de_path = args.input.join(filename);
            let en_path = args.output.join(filename);
            if !de_path.exists() {
                continue;
            }

            let en_file_rel = format!("{}/{}", en_rel.display(), filename);
            let de_file_rel = format!("{}/{}", de_rel.display(), filename);

            let Some(last_commit) = git_last_commit(repo_dir, &en_file_rel) else {
                continue; // not yet committed, skip
            };

            let Some(old_de_content) = git_show(repo_dir, &last_commit, &de_file_rel) else {
                continue; // German file didn't exist at that commit
            };

            let current_de_content =
                fs::read_to_string(&de_path).expect("Failed to read German file");
            let changed = detect_changed_remarks(&old_de_content, &current_de_content);

            if changed.is_empty() {
                continue;
            }

            eprintln!(
                "  {}: {} remark(s) changed since last translation",
                filename,
                changed.len()
            );

            let (_, current_de_remarks) = parse_document(&current_de_content);
            let en_content =
                fs::read_to_string(&en_path).expect("Failed to read English file");
            let (en_preamble, mut en_remarks) = parse_document(&en_content);

            let mut file_changed = false;
            for &(idx, ref anchor) in &changed {
                if idx >= current_de_remarks.len() || idx >= en_remarks.len() {
                    continue;
                }
                if should_skip_remark(&skip_remarks, filename, anchor) {
                    continue;
                }

                eprint!("    {}:{}...", filename, anchor);

                let de_remark = &current_de_remarks[idx];
                let (text_with_placeholders, math_blocks) = extract_math(&de_remark.body);
                let glossary_section = glossary.filter_for(&de_remark.body);
                let empty_context: Vec<String> = Vec::new();

                let best_body = translate_remark(
                    &client,
                    &args.ollama_url,
                    &args.model,
                    &system_msg,
                    &glossary_section,
                    &empty_context,
                    &text_with_placeholders,
                    &math_blocks,
                    de_remark,
                    filename,
                    idx,
                    anchor,
                    args.verbose,
                    args.num_ctx,
                    args.emphasis_tolerance,
                );

                if best_body != en_remarks[idx].body {
                    en_remarks[idx] = Remark {
                        heading: de_remark.heading.clone(),
                        body: best_body,
                    };
                    file_changed = true;
                }
            }

            if file_changed {
                let mut output = en_preamble;
                for r in &en_remarks {
                    output.push_str(&format!("\n{}\n\n{}\n", r.heading, r.body));
                }
                fs::write(&en_path, &output).expect("Failed to write updated file");
            }
        }
        eprintln!();
    }

    // Phase 2: Translate new docs
    let remaining = count_remaining_remarks(&doc_files, &args.input, &args.output);
    eprintln!(
        "{} remarks remaining across untranslated documents",
        remaining
    );
    let mut estimator = TimeEstimator::new(remaining);
    let total = doc_files.len();
    for (file_idx, filename) in doc_files.iter().enumerate() {
        let path = args.input.join(filename);
        if !path.exists() {
            eprintln!(
                "[{}/{}] Skipping {} (not found)",
                file_idx + 1,
                total,
                filename
            );
            continue;
        }

        let out_path = args.output.join(filename);
        if out_path.exists() {
            eprintln!(
                "[{}/{}] Skipping {} (already exists)",
                file_idx + 1,
                total,
                filename
            );
            continue;
        }

        let content = fs::read_to_string(&path).expect("Failed to read file");
        let (preamble, remarks) = parse_document(&content);
        let remark_count = remarks.len();

        // Check for a translated sibling to reuse translations from
        let reuse_map = find_translated_sibling(filename, &args.input, &args.output)
            .map(|sibling| {
                let map = build_reuse_map(
                    &args.input.join(&sibling),
                    &args.output.join(&sibling),
                );
                eprintln!(
                    "  Found sibling {} ({} reusable remarks)",
                    sibling,
                    map.len
                );
                map
            });

        let tmp_path = out_path.with_extension("md.partial");

        let mut skip = 0;
        let mut context_window: VecDeque<String> = VecDeque::new();
        let mut file;

        if tmp_path.exists() {
            let partial = fs::read_to_string(&tmp_path).expect("Failed to read partial file");
            let (_, done_remarks) = parse_document(&partial);
            skip = done_remarks.len();
            for r in done_remarks.iter().rev().take(10).rev() {
                if !r.body.trim().is_empty() {
                    context_window.push_back(r.body.clone());
                }
            }
            eprintln!(
                "[{}/{}] Resuming {} ({}/{} remarks done)...",
                file_idx + 1,
                total,
                filename,
                skip,
                remark_count
            );
            file = fs::OpenOptions::new()
                .append(true)
                .open(&tmp_path)
                .expect("Failed to open partial file for appending");
        } else {
            let reusable = if let Some(ref map) = reuse_map {
                remarks.iter().filter(|r| map.lookup(&r.body).is_some()).count()
            } else {
                0
            };
            if reusable > 0 {
                eprintln!(
                    "[{}/{}] Translating {} ({} remarks, {} reusable, {} to translate)...",
                    file_idx + 1, total, filename, remark_count, reusable,
                    remark_count - reusable
                );
            } else {
                eprintln!("[{}/{}] Translating {} ({} remarks)...",
                    file_idx + 1, total, filename, remark_count);
            }
            file = fs::File::create(&tmp_path).expect("Failed to create output file");
            std::io::Write::write_fmt(&mut file, format_args!("{}", preamble))
                .expect("Failed to write preamble");
        }

        let mut reused_count = 0usize;
        let mut i = skip;
        while i < remark_count {
            // Skip empty remarks
            if remarks[i].body.trim().is_empty() {
                write_remark(&mut file, &remarks[i].heading, &remarks[i].body);
                estimator.record(0.0);
                eprintln!("  remark {}/{} (empty, skipped)", i + 1, remark_count);
                i += 1;
                continue;
            }

            // Check skip list (use original verbatim)
            let remark_id_for_skip = anchor_from_doc_heading(&remarks[i].heading);
            if should_skip_remark(&skip_remarks, filename, &remark_id_for_skip) {
                write_remark(&mut file, &remarks[i].heading, &remarks[i].body);
                estimator.record(0.0);
                eprintln!("  remark {}/{} (skipped, on skip list)", i + 1, remark_count);
                i += 1;
                continue;
            }

            // Check for reusable translation from sibling
            if let Some(ref map) = reuse_map {
                if let Some(en_body) = map.lookup(&remarks[i].body) {
                    write_remark(&mut file, &remarks[i].heading, en_body);
                    estimator.record(0.0);
                    reused_count += 1;
                    context_window.push_back(en_body.to_string());
                    while context_window.len() > 10 {
                        context_window.pop_front();
                    }
                    i += 1;
                    continue;
                }
            }

            // Collect a batch of consecutive non-empty remarks within budget
            let mut batch_chars = 0;
            let batch_start = i;
            while i < remark_count {
                let body = &remarks[i].body;
                if body.trim().is_empty() {
                    break;
                }
                // Don't batch a remark that can be reused from sibling
                if i > batch_start {
                    if let Some(ref map) = reuse_map {
                        if map.lookup(body).is_some() {
                            break;
                        }
                    }
                }
                if i > batch_start && batch_chars + body.len() > max_batch_chars {
                    break;
                }
                batch_chars += body.len();
                i += 1;
            }
            let batch_end = i;
            let batch_size = batch_end - batch_start;

            let eta = estimator
                .estimate_remaining()
                .unwrap_or_else(|| "calculating...".to_string());
            if batch_size == 1 {
                eprint!(
                    "  remark {}/{} [ETA {}]...",
                    batch_start + 1,
                    remark_count,
                    eta
                );
            } else {
                eprint!(
                    "  remarks {}-{}/{} [ETA {}]...",
                    batch_start + 1,
                    batch_end,
                    remark_count,
                    eta
                );
            }

            let start = Instant::now();
            let context = adaptive_context(&context_window, max_context_chars);

            if batch_size == 1 {
                // Single remark — use translate_remark directly
                let remark = &remarks[batch_start];
                let (text_with_placeholders, math_blocks) = extract_math(&remark.body);
                let remark_id = anchor_from_doc_heading(&remark.heading);
                let glossary_section = glossary.filter_for(&remark.body);

                let best_body = translate_remark(
                    &client,
                    &args.ollama_url,
                    &args.model,
                    &system_msg,
                    &glossary_section,
                    &context,
                    &text_with_placeholders,
                    &math_blocks,
                    remark,
                    filename,
                    batch_start,
                    &remark_id,
                    args.verbose,
                    args.num_ctx,
                    args.emphasis_tolerance,
                );

                let elapsed = start.elapsed().as_secs_f64();
                estimator.record(elapsed);
                write_remark(&mut file, &remark.heading, &best_body);
                if best_body != remark.body {
                    context_window.push_back(best_body);
                }
            } else {
                // Batch: extract math + emphasis per remark, combine into numbered format
                let mut per_remark_math: Vec<Vec<String>> = Vec::new();
                let mut combined_glossary = String::new();
                let mut batch_text = String::new();
                let mut math_offset = 0;

                for (bi, idx) in (batch_start..batch_end).enumerate() {
                    let body = &remarks[idx].body;
                    // Extract math with offset numbering so placeholders don't collide
                    let (mut text, blocks) = extract_math(body);
                    // Renumber math placeholders to be globally unique within batch
                    for (j, _) in blocks.iter().enumerate() {
                        let old = format!("\u{27E6}MATH:{}\u{27E7}", j + 1);
                        let new = format!("\u{27E6}MATH:{}\u{27E7}", math_offset + j + 1);
                        if old != new {
                            text = text.replace(&old, &new);
                        }
                    }
                    per_remark_math.push(blocks);
                    math_offset += per_remark_math.last().unwrap().len();

                    // Convert emphasis to HTML
                    let html_text = emphasis_to_html(&text);
                    combined_glossary.push_str(&glossary.filter_for(body));

                    if bi > 0 {
                        batch_text.push_str("\n\n");
                    }
                    batch_text.push_str(&format!("[{}]\n{}", bi + 1, html_text));
                }

                // Build user message
                let mut user_msg = String::new();
                if !combined_glossary.is_empty() {
                    user_msg.push_str(
                        "Use the following glossary for established translations \
                         of key philosophical terms. Follow these conventions strictly:\n\n",
                    );
                    user_msg.push_str(&combined_glossary);
                    user_msg.push_str("\n\n");
                }
                if !context.is_empty() {
                    user_msg.push_str("Context (already translated preceding remarks):\n\n");
                    for (ci, ctx) in context.iter().enumerate() {
                        if ci > 0 {
                            user_msg.push_str("\n\n---\n\n");
                        }
                        user_msg.push_str(ctx);
                    }
                    user_msg.push_str("\n\n---\n\n");
                }
                user_msg.push_str(&format!(
                    "Translate each of the following {} numbered German passages to English. \
                     Output your translations in the same numbered format ([1], [2], etc.).\n\
                     Preserve all HTML tags (<em>, </em>, etc.) exactly.\n\n{}",
                    batch_size, batch_text
                ));

                if args.verbose {
                    eprintln!(
                        "\n--- SYSTEM PROMPT ---\n{}\n--- USER MSG ---\n{}\n---",
                        system_msg, user_msg
                    );
                }

                match call_ollama(
                    &client,
                    &args.ollama_url,
                    &args.model,
                    &system_msg,
                    &user_msg,
                    args.num_ctx,
                ) {
                    Ok(response) => {
                        if args.verbose {
                            eprintln!("--- EN ---\n{}\n---", response);
                        }
                        // Split response by [N] markers
                        let split_re =
                            regex::Regex::new(r"(?m)^\[(\d+)\]\s*\n?").unwrap();
                        let parts: Vec<&str> = split_re.split(&response).collect();
                        let numbers: Vec<usize> = split_re
                            .captures_iter(&response)
                            .filter_map(|c| c[1].parse::<usize>().ok())
                            .collect();

                        // Check if we got the right number of parts
                        let translated_parts: Vec<(usize, &str)> =
                            numbers.iter().copied().zip(parts.iter().skip(1).copied()).collect();

                        if translated_parts.len() == batch_size {
                            let elapsed = start.elapsed().as_secs_f64();
                            estimator.record_n(elapsed, batch_size);

                            let mut math_idx = 0;
                            for (bi, idx) in (batch_start..batch_end).enumerate() {
                                let (_, translated) = translated_parts[bi];
                                let restored_em = fix_emphasis_markers(&emphasis_from_html(translated.trim()), &remarks[idx].body);
                                // Restore math with renumbered placeholders
                                let blocks = &per_remark_math[bi];
                                let mut restored = restored_em;
                                for (j, block) in blocks.iter().enumerate() {
                                    let placeholder = format!(
                                        "\u{27E6}MATH:{}\u{27E7}",
                                        math_idx + j + 1
                                    );
                                    restored = restored.replacen(&placeholder, block, 1);
                                }
                                math_idx += blocks.len();

                                write_remark(&mut file, &remarks[idx].heading, &restored);
                                context_window.push_back(restored);
                            }
                            eprintln!(" done ({:.1}s, {} remarks)", elapsed, batch_size);
                        } else {
                            // Batch split failed — fall back to individual translation
                            eprintln!(
                                " batch split failed (got {} parts, expected {}), falling back...",
                                translated_parts.len(),
                                batch_size
                            );
                            let elapsed_batch = start.elapsed().as_secs_f64();
                            for idx in batch_start..batch_end {
                                let remark = &remarks[idx];
                                let remark_start = Instant::now();
                                eprint!("  remark {}/{}...", idx + 1, remark_count);
                                let (text_with_placeholders, math_blocks) =
                                    extract_math(&remark.body);
                                let remark_id = anchor_from_doc_heading(&remark.heading);
                                let glossary_section = glossary.filter_for(&remark.body);
                                let ctx = adaptive_context(&context_window, max_context_chars);

                                let best_body = translate_remark(
                                    &client,
                                    &args.ollama_url,
                                    &args.model,
                                    &system_msg,
                                    &glossary_section,
                                    &ctx,
                                    &text_with_placeholders,
                                    &math_blocks,
                                    remark,
                                    filename,
                                    idx,
                                    &remark_id,
                                    args.verbose,
                                    args.num_ctx,
                                    args.emphasis_tolerance,
                                );

                                let elapsed = remark_start.elapsed().as_secs_f64();
                                estimator.record(elapsed);
                                write_remark(&mut file, &remark.heading, &best_body);
                                if best_body != remark.body {
                                    context_window.push_back(best_body);
                                }
                            }
                            let _ = elapsed_batch; // suppress warning
                        }
                    }
                    Err(e) => {
                        eprintln!(" BATCH ERROR: {}, falling back...", e);
                        for idx in batch_start..batch_end {
                            let remark = &remarks[idx];
                            eprint!("  remark {}/{}...", idx + 1, remark_count);
                            let (text_with_placeholders, math_blocks) =
                                extract_math(&remark.body);
                            let remark_id = anchor_from_doc_heading(&remark.heading);
                            let glossary_section = glossary.filter_for(&remark.body);
                            let ctx = adaptive_context(&context_window, max_context_chars);

                            let best_body = translate_remark(
                                &client,
                                &args.ollama_url,
                                &args.model,
                                &system_msg,
                                &glossary_section,
                                &ctx,
                                &text_with_placeholders,
                                &math_blocks,
                                remark,
                                filename,
                                idx,
                                &remark_id,
                                args.verbose,
                                args.num_ctx,
                                args.emphasis_tolerance,
                            );

                            estimator.record(0.0);
                            write_remark(&mut file, &remark.heading, &best_body);
                            if best_body != remark.body {
                                context_window.push_back(best_body);
                            }
                        }
                    }
                }
            }
            // Keep context window bounded
            while context_window.len() > 10 {
                context_window.pop_front();
            }
        }

        fs::rename(&tmp_path, &out_path).expect("Failed to rename completed file");
        if reused_count > 0 {
            eprintln!(
                "  -> {} ({} remarks reused from sibling)",
                out_path.display(),
                reused_count
            );
        } else {
            eprintln!("  -> {}", out_path.display());
        }

        // Document-level verify + fix loop
        if !args.no_verify {
            verify_and_fix_doc(
                filename,
                &path,
                &out_path,
                &client,
                &args.ollama_url,
                &args.model,
                &system_msg,
                &glossary,
                args.verbose,
                args.num_ctx,
                args.emphasis_tolerance,
                &skip_remarks,
            );
        }
    }

    // Phase 2: Assemble works from translated docs
    if work_files.is_empty() {
        eprintln!("No work files to assemble.");
    } else {
        eprintln!("\nAssembling {} work file(s)...", work_files.len());
        let url_map = build_remark_url_map(&args.output, &args.input);

        for filename in &work_files {
            let work_path = args.input.join(filename);
            let out_path = args.output.join(filename);
            if !work_path.exists() {
                eprintln!("  Skipping {} (not found)", filename);
                continue;
            }
            eprint!("  {}...", filename);
            let missing = assemble_work(&work_path, &url_map, &out_path);
            if missing > 0 {
                eprintln!(" done ({} remarks missing translations)", missing);
            } else {
                eprintln!(" done");
            }
        }
    }

    eprintln!("\nDone. Translated files are in {}", args.output.display());
}
