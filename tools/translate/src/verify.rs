use crate::common::*;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

pub struct VerifyArgs {
    pub input: PathBuf,
    pub translated: PathBuf,
    pub emphasis_tolerance: usize,
}

pub struct Issue {
    pub file: String,
    pub remark_id: String,
    pub check: &'static str,
    pub description: String,
}

/// Extract all <math>...</math> blocks from text.
fn extract_math_blocks(text: &str) -> Vec<String> {
    let re = regex::Regex::new(r"(?s)<math[\s>].*?</math>").unwrap();
    re.find_iter(text).map(|m| m.as_str().to_string()).collect()
}

/// Extract all HTML tags (excluding math) from text.
fn extract_html_tags(text: &str) -> Vec<String> {
    let no_math = regex::Regex::new(r"(?s)<math[\s>].*?</math>")
        .unwrap()
        .replace_all(text, "")
        .into_owned();
    let re = regex::Regex::new(r"<[^>]+>").unwrap();
    re.find_iter(&no_math)
        .map(|m| m.as_str().to_string())
        .collect()
}

/// Extract all image references from text.
fn extract_images(text: &str) -> Vec<String> {
    let re = regex::Regex::new(r"!\[[^\]]*\]\([^)]+\)").unwrap();
    re.find_iter(text).map(|m| m.as_str().to_string()).collect()
}

/// Strip all markup/formatting, leaving only natural language text.
fn plain_text(text: &str) -> String {
    let no_math = regex::Regex::new(r"(?s)<math[\s>].*?</math>")
        .unwrap()
        .replace_all(text, "")
        .into_owned();
    let no_html = regex::Regex::new(r"<[^>]+>")
        .unwrap()
        .replace_all(&no_math, "")
        .into_owned();
    no_html
        .replace('_', "")
        .replace("**", "")
        .replace("![", "")
        .replace("](", "")
        .replace(')', "")
}

/// Check for undirected (straight) double quotes in prose (ignoring HTML tags and math).
fn check_quotes(text: &str) -> Vec<String> {
    let math_re = regex::Regex::new(r"(?s)<math[\s>].*?</math>").unwrap();
    let tag_re = regex::Regex::new(r"<[^>]+>").unwrap();
    let mut hits = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let no_math = math_re.replace_all(line, "");
        let no_tags = tag_re.replace_all(&no_math, "");
        if no_tags.contains('"') {
            let context: String = line.chars().take(80).collect();
            hits.push(format!("line {}: {}", i + 1, context));
        }
    }
    hits
}

/// Count German-specific characters.
fn german_char_count(text: &str) -> usize {
    text.chars()
        .filter(|c| matches!(*c, 'ä' | 'ö' | 'ü' | 'Ä' | 'Ö' | 'Ü' | 'ß'))
        .count()
}

pub fn verify_remark(
    file_name: &str,
    _idx: usize,
    remark_id: &str,
    german: &Remark,
    english: &Remark,
    issues: &mut Vec<Issue>,
    emphasis_tolerance: usize,
) {
    let de_body = &german.body;
    let en_body = &english.body;

    // 1. Structural check — math blocks must be identical
    let de_math = extract_math_blocks(de_body);
    let en_math = extract_math_blocks(en_body);
    if de_math.len() != en_math.len() {
        issues.push(Issue {
            file: file_name.to_string(),
            remark_id: remark_id.to_string(),
            check: "structure",
            description: format!(
                "Math block count mismatch: DE has {}, EN has {}",
                de_math.len(),
                en_math.len()
            ),
        });
    } else {
        for (i, (de_m, en_m)) in de_math.iter().zip(en_math.iter()).enumerate() {
            if de_m != en_m {
                issues.push(Issue {
                    file: file_name.to_string(),
                    remark_id: remark_id.to_string(),
                    check: "structure",
                    description: format!("Math block {} differs between DE and EN", i + 1),
                });
            }
        }
    }

    // Structural check — HTML tags (outside math) must match
    let de_tags = extract_html_tags(de_body);
    let en_tags = extract_html_tags(en_body);
    if de_tags != en_tags {
        issues.push(Issue {
            file: file_name.to_string(),
            remark_id: remark_id.to_string(),
            check: "structure",
            description: format!("HTML tags differ: DE {:?} vs EN {:?}", de_tags, en_tags),
        });
    }

    // Structural check — image references must be identical
    let de_imgs = extract_images(de_body);
    let en_imgs = extract_images(en_body);
    if de_imgs != en_imgs {
        issues.push(Issue {
            file: file_name.to_string(),
            remark_id: remark_id.to_string(),
            check: "structure",
            description: format!(
                "Image references differ: DE has {}, EN has {}",
                de_imgs.len(),
                en_imgs.len()
            ),
        });
    }

    // 2. Emphasis marker check — _ and * counts must be within tolerance
    let de_underscores = de_body.chars().filter(|&c| c == '_').count();
    let en_underscores = en_body.chars().filter(|&c| c == '_').count();
    let us_diff = (de_underscores as isize - en_underscores as isize).unsigned_abs();
    if us_diff > emphasis_tolerance {
        issues.push(Issue {
            file: file_name.to_string(),
            remark_id: remark_id.to_string(),
            check: "emphasis",
            description: format!(
                "Underscore count mismatch: DE has {}, EN has {}",
                de_underscores, en_underscores
            ),
        });
    }
    let de_asterisks = de_body.chars().filter(|&c| c == '*').count();
    let en_asterisks = en_body.chars().filter(|&c| c == '*').count();
    if de_asterisks != en_asterisks {
        issues.push(Issue {
            file: file_name.to_string(),
            remark_id: remark_id.to_string(),
            check: "emphasis",
            description: format!(
                "Asterisk count mismatch: DE has {}, EN has {}",
                de_asterisks, en_asterisks
            ),
        });
    }

    // 3. Quotation mark check — no straight double quotes in English
    for hit in check_quotes(en_body) {
        issues.push(Issue {
            file: file_name.to_string(),
            remark_id: remark_id.to_string(),
            check: "quotes",
            description: format!("Undirected double quote: {}", hit),
        });
    }

    // 5. Length ratio
    let de_plain = plain_text(de_body);
    let en_plain = plain_text(en_body);
    let de_len = de_plain.trim().len();
    let en_len = en_plain.trim().len();
    if de_len > 20 {
        let ratio = en_len as f64 / de_len as f64;
        if ratio < 0.3 {
            issues.push(Issue {
                file: file_name.to_string(),
                remark_id: remark_id.to_string(),
                check: "length",
                description: format!(
                    "Possible truncation: ratio {:.2} (DE {} chars, EN {} chars)",
                    ratio, de_len, en_len
                ),
            });
        } else if ratio > 2.0 {
            issues.push(Issue {
                file: file_name.to_string(),
                remark_id: remark_id.to_string(),
                check: "length",
                description: format!(
                    "Possible hallucination: ratio {:.2} (DE {} chars, EN {} chars)",
                    ratio, de_len, en_len
                ),
            });
        }
    }

    // 6. Untranslated detection
    let gc = german_char_count(en_body);
    if gc > 3 {
        issues.push(Issue {
            file: file_name.to_string(),
            remark_id: remark_id.to_string(),
            check: "untranslated",
            description: format!(
                "English contains {} German-specific characters (ä/ö/ü/ß)",
                gc
            ),
        });
    }

}

pub fn run(args: &VerifyArgs) -> Vec<Issue> {
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

    // Collect translated files: .md and .md.partial, skip W-* and index
    let mut files: Vec<(String, std::path::PathBuf)> = Vec::new();
    for entry in fs::read_dir(&args.translated).expect("Failed to read translated dir") {
        let entry = entry.unwrap();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("W-") || name == "index.md" {
            continue;
        }
        if name.ends_with(".md") {
            files.push((name, entry.path()));
        } else if name.ends_with(".md.partial") {
            // Use the base .md name for matching against the German original
            let base_name = name.replace(".partial", "");
            files.push((base_name, entry.path()));
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));

    let mut all_issues: Vec<Issue> = Vec::new();

    for (base_name, en_path) in &files {
        let de_path = args.input.join(base_name);
        if !de_path.exists() {
            eprintln!("Skipping {} (no German original found)", base_name);
            continue;
        }

        let de_content = fs::read_to_string(&de_path).expect("Failed to read German file");
        let en_content = fs::read_to_string(en_path).expect("Failed to read English file");

        let (_, de_remarks) = parse_document(&de_content);
        let (_, en_remarks) = parse_document(&en_content);

        // For partial files, we may have fewer English remarks — only verify what exists
        let is_partial = en_path.to_string_lossy().ends_with(".partial");
        if !is_partial && de_remarks.len() != en_remarks.len() {
            all_issues.push(Issue {
                file: base_name.clone(),
                remark_id: String::new(),
                check: "structure",
                description: format!(
                    "Remark count mismatch: German has {}, English has {}",
                    de_remarks.len(),
                    en_remarks.len()
                ),
            });
            continue;
        }

        let check_count = en_remarks.len().min(de_remarks.len());
        let suffix = if is_partial {
            format!(" (partial, {}/{} remarks)", check_count, de_remarks.len())
        } else {
            String::new()
        };
        eprintln!("Checking {}{}", base_name, suffix);

        let skip_remarks = load_skip_remarks(std::path::Path::new("."));
        for (i, (de, en)) in de_remarks.iter().zip(en_remarks.iter()).enumerate() {
            let remark_id = anchor_from_doc_heading(&de.heading);
            if should_skip_remark(&skip_remarks, base_name, &remark_id) {
                continue;
            }
            verify_remark(base_name, i, &remark_id, de, en, &mut all_issues, args.emphasis_tolerance);
        }
    }

    // Report
    if all_issues.is_empty() {
        eprintln!("No issues found.");
    } else {
        for issue in &all_issues {
            println!(
                "{}:{} [{}] {}",
                issue.file, issue.remark_id, issue.check, issue.description
            );
        }
        eprintln!(
            "\n{} issue(s) found across {} file(s).",
            all_issues.len(),
            all_issues
                .iter()
                .map(|i| &i.file)
                .collect::<HashSet<_>>()
                .len()
        );
    }

    all_issues
}
