use crate::common::*;
use crate::verify::{ignore_words_for, load_allow_words, load_ignore_words, verify_remark, Issue};
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
/// Docs with no English file at all (`md-en/<doc>` absent) are skipped entirely: only
/// docs that have been translated are held to completeness.
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
    let allow_words = load_allow_words(Path::new("."));

    // Document order from all.md (works are assembled from these docs, so checking the
    // docs covers the works' content too, provided the works are assembled from them:
    // see `check_works` below).
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
    let mut untranslated_docs: Vec<&str> = Vec::new();

    for filename in &docs {
        let de_path = args.input.join(filename);
        if !de_path.exists() {
            continue;
        }
        let de_content = std::fs::read_to_string(&de_path).expect("Failed to read German file");
        let (_, de_remarks) = parse_document(&de_content);

        let en_path = args.translated.join(filename);

        // Whole doc untranslated (no md-en/<doc> at all): not part of the translated
        // corpus yet, so skip it. Missing remarks are only enforced within docs that
        // have been (at least partially) translated.
        if !en_path.exists() {
            untranslated_docs.push(filename.as_str());
            continue;
        }

        let en_content = std::fs::read_to_string(&en_path).expect("Failed to read English file");
        let (_, en_remarks) = parse_document(&en_content);
        let en_by_anchor: HashMap<String, &Remark> = en_remarks
            .iter()
            .map(|r| (anchor_from_doc_heading(&r.heading), r))
            .collect();
        // Duplicate anchors occur in the corpus. When headings align exactly, use document
        // order rather than the last remark stored in the anchor map.
        let aligned = de_remarks.len() == en_remarks.len()
            && de_remarks.iter().zip(&en_remarks).all(|(de, en)| de.heading == en.heading);

        // Missing remarks + quality verification of existing pairs.
        for (i, de) in de_remarks.iter().enumerate() {
            let rid = anchor_from_doc_heading(&de.heading);
            if rid.is_empty() || should_skip_remark(&skip_remarks, filename, &rid) {
                continue;
            }
            let translated = if aligned {
                en_remarks.get(i)
            } else {
                en_by_anchor.get(&rid).copied()
            };
            match translated {
                None => missing.push(format!("{}:{}", filename, rid)),
                Some(en) => verify_remark(
                    filename,
                    i,
                    &rid,
                    de,
                    en,
                    &mut issues,
                    args.emphasis_tolerance,
                    &ignore_words_for(&ignore_words, &allow_words, filename, &rid),
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

    let formulas = stale_formulas(&args.input, &args.translated);
    let (work_missing, works_outdated) = check_works(&args.input, &args.translated);

    // Report.
    if !untranslated_docs.is_empty() {
        eprintln!(
            "Skipping {} untranslated doc(s) without an English file: {}",
            untranslated_docs.len(),
            untranslated_docs.join(", ")
        );
    }
    let broken = !missing.is_empty()
        || !stale.is_empty()
        || !issues.is_empty()
        || !formulas.is_empty()
        || !work_missing.is_empty()
        || !works_outdated.is_empty();
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

    if !formulas.is_empty() {
        eprintln!(
            "\nFormulas in English files that their German file doesn't have (outdated copies; \
             replace them with the German's current markup) ({}):",
            formulas.len()
        );
        for f in &formulas {
            println!("FORMULA {}", f);
        }
    }

    if !work_missing.is_empty() {
        eprintln!(
            "\nWork remarks whose document is translated but that the assembly can't find in \
             it, so the work keeps the German ({}):",
            work_missing.len()
        );
        for w in &work_missing {
            println!("WORK-MISSING {}", w);
        }
    }
    if !works_outdated.is_empty() {
        eprintln!(
            "\nWorks that differ from a fresh assembly from the documents' translations \
             (run `make fix`) ({}):",
            works_outdated.len()
        );
        for w in &works_outdated {
            println!("WORK-OUTDATED {}", w);
        }
    }

    if broken {
        eprintln!(
            "\nBuild is broken: {} missing, {} stale, {} quality issue(s), {} outdated formula(s), \
             {} work remark(s) not assembled, {} outdated work(s).",
            missing.len(),
            stale.len(),
            issues.len(),
            formulas.len(),
            work_missing.len(),
            works_outdated.len()
        );
        std::process::exit(1);
    }
    eprintln!("All English translations present, up to date, and verified.");
}

/// The translated works (`md-en/W-*.md`), assembled anew from the documents' translations
/// as `make fix` does: the remarks the assembly kept in German although their document has
/// an English file (`W-PR.md:ts-214a1#1.1`), and the works whose file differs from the
/// fresh assembly (edited by hand, or not reassembled since a document changed).
fn check_works(input: &Path, translated: &Path) -> (Vec<String>, Vec<String>) {
    let url_map = build_remark_url_map(translated, input);
    let work_titles = std::fs::read_to_string(input.join("all.md"))
        .map(|c| parse_work_titles(&c))
        .unwrap_or_default();
    let base_titles = build_base_titles(&work_titles, input);
    let mut works: Vec<String> = std::fs::read_dir(translated)
        .expect("Failed to read translated dir")
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|name| name.starts_with("W-") && name.ends_with(".md"))
        .collect();
    works.sort();
    let (mut missing, mut outdated) = (Vec::new(), Vec::new());
    for work in works {
        let Ok(de) = std::fs::read_to_string(input.join(&work)) else { continue };
        let assembled = assemble_work_text(&de, &url_map, &base_titles);
        for heading in &assembled.missing {
            match parse_work_heading(heading) {
                Some((doc, anchor)) if translated.join(format!("{doc}.md")).exists() => {
                    missing.push(format!("{work}:{doc}#{anchor}"))
                }
                Some(_) => {} // the document isn't translated yet
                None => missing.push(format!("{work}:{heading}")),
            }
        }
        let en = std::fs::read_to_string(translated.join(&work)).expect("Failed to read English work");
        if en != assembled.text {
            outdated.push(work);
        }
    }
    (missing, outdated)
}

/// Formulas (math blocks: rendered LaTeX images, inline graphics, legacy MathML) that an
/// English file has but its German file doesn't, as `file:remark alt`. The English copies
/// them verbatim, so any such block is an outdated copy: the formula was re-rendered, laid
/// out anew or broken into pieces since, and the English wasn't updated. Unlike the
/// per-remark verification above, this covers every English file, the works included (which
/// are not assembled from the documents' translations, so they can go stale on their own).
fn stale_formulas(input: &Path, translated: &Path) -> Vec<String> {
    let mut files: Vec<String> = std::fs::read_dir(translated)
        .expect("Failed to read translated dir")
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|name| name.ends_with(".md"))
        .collect();
    files.sort();
    let mut out = Vec::new();
    for filename in files {
        let Ok(de) = std::fs::read_to_string(input.join(&filename)) else { continue };
        let en = std::fs::read_to_string(translated.join(&filename)).expect("Failed to read English file");
        for (remark, block) in formulas_missing_from(&de, &en) {
            out.push(format!("{filename}:{remark} {}", formula_label(block)));
        }
    }
    out
}

/// The math blocks of `en` that don't occur in `de`, with the anchor of the remark they're
/// in (`preamble` before the first remark).
fn formulas_missing_from<'a>(de: &str, en: &'a str) -> Vec<(String, &'a str)> {
    let re = regex::Regex::new(MATH_PATTERN).unwrap();
    let german: std::collections::HashSet<&str> = re.find_iter(de).map(|m| m.as_str()).collect();
    let mut remark = "preamble".to_string();
    let mut out = Vec::new();
    let mut offset = 0;
    for line in en.split_inclusive('\n') {
        if line.starts_with("### ") {
            remark = anchor_from_doc_heading(line.trim_end());
        }
        for m in re.find_iter(line) {
            if !german.contains(m.as_str()) {
                out.push((remark.clone(), &en[offset + m.start()..offset + m.end()]));
            }
        }
        offset += line.len();
    }
    out
}

/// A short label for a math block: its alt text (the LaTeX), else its start.
fn formula_label(block: &str) -> String {
    let alt = regex::Regex::new(r#"alt="([^"]*)""#).unwrap();
    let text = alt.captures(block).map_or(block, |c| c.get(1).unwrap().as_str());
    // The entities the parser writes in alt text.
    let text = [("&#95;", "_"), ("&#42;", "*"), ("&#96;", "`"), ("&lt;", "<"), ("&gt;", ">"), ("&quot;", "\""), ("&amp;", "&")]
        .iter()
        .fold(text.to_string(), |t, (entity, c)| t.replace(entity, c));
    text.chars().take(80).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_outdated_formula_copies() {
        let fresh = r#"<img class="math math-inline" src="../math/a.svg" alt="a=b" style="width:2em">"#;
        let old = r#"<img class="math math-inline" src="../math/a.svg" alt="a=b" style="width:3em">"#;
        let de = format!("### [Ms-1](/ms-1/#1.1)\n\nAlso {fresh}.\n");
        assert!(formulas_missing_from(&de, &format!("### [Ms-1](/ms-1/#1.1)\n\nSo {fresh}.\n")).is_empty());
        let en = format!("### [Ms-1](/ms-1/#1.1)\n\nSo {old}.\n");
        let found = formulas_missing_from(&de, &en);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].1, old);
        assert_eq!(formula_label(found[0].1), "a=b");
    }
}
