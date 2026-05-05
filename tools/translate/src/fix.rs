use crate::common::*;
use crate::verify;
use std::fs;
use std::path::PathBuf;

pub struct FixArgs {
    pub input: PathBuf,
    pub translated: PathBuf,
    pub model: String,
    pub ollama_url: String,
    pub glossary: Option<PathBuf>,
    pub verbose: bool,
}

const MAX_ITERATIONS: usize = 3;

/// Generate LLM-actionable fix instructions from a list of issues for a single remark.
pub fn fix_instructions(issues: &[&verify::Issue], de_body: &str) -> String {
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
                         Your translation must also have exactly {} underscores. \
                         Preserve every _emphasized passage_ using underscores.",
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

pub fn run(args: &FixArgs) {
    let glossary = match &args.glossary {
        Some(path) => {
            let content = fs::read_to_string(path).expect("Failed to read glossary");
            Glossary::parse(&content)
        }
        None => Glossary::empty(),
    };
    let system_msg = translation_system_prompt();
    let client = reqwest::blocking::Client::new();

    let verify_args = verify::VerifyArgs {
        input: args.input.clone(),
        translated: args.translated.clone(),
    };

    for iteration in 1..=MAX_ITERATIONS {
        eprintln!("\n=== Fix iteration {}/{} ===\n", iteration, MAX_ITERATIONS);

        let all_issues = verify::run(&verify_args);

        // Filter to fixable issues in completed .md files
        let fixable: Vec<&verify::Issue> = all_issues
            .iter()
            .filter(|i| {
                let path = args.translated.join(&i.file);
                path.exists() && i.remark_idx > 0
            })
            .collect();

        if fixable.is_empty() {
            eprintln!("\nNo issues to fix.");
            break;
        }

        // Deduplicate: one re-translation per (file, remark)
        let mut fix_targets: Vec<(String, usize, String)> = fixable
            .iter()
            .map(|i| (i.file.clone(), i.remark_idx, i.remark_id.clone()))
            .collect();
        fix_targets.sort();
        fix_targets.dedup();

        eprintln!("\nRe-translating {} remark(s)...", fix_targets.len());

        for (file_name, remark_idx, remark_id) in &fix_targets {
            let de_path = args.input.join(file_name);
            let en_path = args.translated.join(file_name);

            let de_content = fs::read_to_string(&de_path).expect("Failed to read German file");
            let en_content = fs::read_to_string(&en_path).expect("Failed to read English file");

            let (_, de_remarks) = parse_document(&de_content);
            let (en_preamble, mut en_remarks) = parse_document(&en_content);

            let idx = remark_idx - 1;
            if idx >= de_remarks.len() || idx >= en_remarks.len() {
                eprintln!(
                    "  {}:{} — skipping (out of bounds)",
                    file_name, remark_id
                );
                continue;
            }

            let issues_for_remark: Vec<&verify::Issue> = all_issues
                .iter()
                .filter(|i| &i.file == file_name && i.remark_idx == *remark_idx)
                .collect();
            let instructions = fix_instructions(&issues_for_remark, &de_remarks[idx].body);

            eprint!("  {}:{}...", file_name, remark_id);

            let de_prose = extractable_text(&de_remarks[idx].body);
            let (de_with_placeholders, math_blocks) = extract_math(&de_prose);
            let glossary_section = glossary.filter_for(&de_prose);

            let mut user_msg = String::new();
            if !glossary_section.is_empty() {
                user_msg.push_str(
                    "Use the following glossary for established translations \
                     of key philosophical terms. Follow these conventions strictly:\n\n",
                );
                user_msg.push_str(&glossary_section);
                user_msg.push_str("\n\n");
            }
            user_msg.push_str(&format!(
                "Translate the following German text to English.\n\n\
                 IMPORTANT — a previous translation attempt had problems. \
                 Make sure your translation avoids them:\n\n\
                 {}\n\n\
                 German text:\n\n{}",
                instructions, de_with_placeholders
            ));

            if args.verbose {
                eprintln!("\n--- SYSTEM PROMPT ---\n{}\n--- USER MSG ---\n{}\n--- DE ---\n{}\n---",
                    system_msg, user_msg, de_prose);
            }

            match call_ollama(
                &client,
                &args.ollama_url,
                &args.model,
                &system_msg,
                &user_msg,
            ) {
                Ok(fixed) => {
                    if args.verbose {
                        eprintln!("--- EN ---\n{}\n---", fixed);
                    }
                    let restored = restore_math(&fixed, &math_blocks);
                    let new_body = reconstruct_body(&de_remarks[idx].body, &restored);
                    en_remarks[idx] = Remark {
                        heading: en_remarks[idx].heading.clone(),
                        body: new_body,
                    };
                    eprintln!(" done");
                }
                Err(e) => {
                    eprintln!(" ERROR: {}", e);
                    continue;
                }
            }

            // Write back the entire file
            let mut output = en_preamble.clone();
            for r in &en_remarks {
                output.push_str(&format!("\n{}\n\n{}\n", r.heading, r.body));
            }
            fs::write(&en_path, &output).expect("Failed to write fixed file");
        }
    }

    // Reassemble all work files from translated docs
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

    eprintln!("\nDone.");
}
