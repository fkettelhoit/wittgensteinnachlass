use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::LazyLock;

/// A Markdown link: `[text](url)`, where the text may contain escaped brackets.
static LINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[([^\]]*(?:\\.[^\]]*)*)\]\(([^)]+)\)").unwrap());
static DATE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\d{2})\.(\d{2})\.(\d{4})$").unwrap());

pub struct WorkRemark {
    pub source_doc: String,
    pub source_anchor: String,
}

pub struct Work {
    pub title: String,
    pub remarks: Vec<WorkRemark>,
}

/// What appears between two consecutive remarks in a source document.
pub enum RemarkBreak {
    /// Normal continuation (no separator, no large date gap)
    None,
    /// A `---` separator line within the preceding remark
    Separator,
    /// A gap of >30 days between dated remarks
    DateGap,
}

pub struct SourceDoc {
    pub anchors: Vec<String>,
    /// Bergen-style siglum of each remark (`Ms-115,118[2]et119[1]`), as assigned by
    /// tools/similar: repeated sigla get `-2`, `-3`, … in document order.
    pub sigla: Vec<String>,
    /// For each remark (except the first), what kind of break precedes it.
    pub breaks: Vec<RemarkBreak>,
    /// For each remark, its backlinks to work pages as (work slug, anchor on the work
    /// page): `[PI](/w-pi/#ts-227a-1.1)` → ("w-pi", "ts-227a-1.1"). The anchor is that
    /// of the version the work page shows, which may be another document's copy.
    pub work_links: Vec<Vec<(String, String)>>,
}

pub struct Correspondence {
    pub work_idx: usize,
    pub doc_name: String,
    pub source_idx: usize,
}

/// Compute the anchor ID from a remark heading's page refs.
/// E.g., heading `### [1\[1\]](url)` → page ref text `1[1]` → anchor `1.1`
/// Skips work reference links (internal URLs starting with `/`) and only
/// uses facsimile links (external URLs) for the anchor.
fn anchor_from_doc_heading(heading: &str) -> String {
    // Extract link text and URL pairs from the heading
    let link_re = &*LINK_RE;
    let mut parts = Vec::new();
    for cap in link_re.captures_iter(heading) {
        let url = &cap[2];
        // Skip internal links (work references like /w-lw-1/#...)
        if url.starts_with('/') {
            continue;
        }
        let text = cap[1].replace("\\[", "[").replace("\\]", "]");
        parts.push(text);
    }
    let joined = parts.join(",");
    // Convert to anchor format: replace [ with ., drop ], join with +
    // But actually we need to match the remark_anchor_id format from the parser:
    // split by comma (which separates pages), then replace [ with . and drop ]
    joined
        .split(',')
        .map(|seg| seg.replace('[', ".").replace(']', ""))
        .collect::<Vec<_>>()
        .join("+")
}

/// The work backlinks of a document remark heading, as (work slug, work page anchor).
fn work_links_of_heading(heading: &str) -> Vec<(String, String)> {
    LINK_RE
        .captures_iter(heading)
        .filter_map(|cap| {
            let (slug, anchor) = cap[2].strip_prefix('/')?.split_once("/#")?;
            Some((slug.to_string(), anchor.to_string()))
        })
        .collect()
}

/// The abbreviation of a work, from its filename stem, as the site shows it (see
/// `work_display_label` in the parser): "W-PI" → "PI", "W-PG-1-App-3" → "PG I App III",
/// "W-LW-2-MS-169" → "LW II MS 169".
pub fn work_label(stem: &str) -> String {
    let base = stem.strip_prefix("W-").unwrap_or(stem);
    base.split('-')
        .map(|seg| match seg {
            "1" => "I",
            "2" => "II",
            "3" => "III",
            "4" => "IV",
            "5" => "V",
            "6" => "VI",
            "7" => "VII",
            "8" => "VIII",
            "9" => "IX",
            "10" => "X",
            other => other,
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Parse a work markdown file to extract source references.
pub fn parse_work(path: &Path) -> Work {
    let content = fs::read_to_string(path).expect("Failed to read work file");
    let title = work_label(&path.file_stem().unwrap().to_string_lossy());

    // Parse each ### heading to extract source doc and anchor
    let heading_re = Regex::new(r"### \[([^\]]+)\]\(/[^)]+/#([^)]+)\)").unwrap();
    let mut remarks = Vec::new();

    for line in content.lines() {
        if let Some(cap) = heading_re.captures(line) {
            remarks.push(WorkRemark {
                source_doc: cap[1].to_string(),
                source_anchor: cap[2].to_string(),
            });
        }
    }

    Work { title, remarks }
}

/// Facsimile link labels of a remark heading (`118[2]`), skipping internal work links.
fn fac_labels(heading: &str) -> Vec<String> {
    LINK_RE
        .captures_iter(heading)
        .filter(|cap| !cap[2].starts_with('/'))
        .map(|cap| cap[1].replace("\\[", "[").replace("\\]", "]"))
        .collect()
}

/// Parse a DD.MM.YYYY date string into a day count for comparison.
fn parse_date(s: &str) -> Option<i64> {
    let cap = DATE_RE.captures(s.trim())?;
    let d: i64 = cap[1].parse().ok()?;
    let m: i64 = cap[2].parse().ok()?;
    let y: i64 = cap[3].parse().ok()?;
    // Rough day count — exact accuracy not needed, just >30 day detection
    Some(y * 365 + m * 30 + d)
}

/// Parse a source document to get remark anchors and inter-remark breaks.
pub fn parse_source_doc(path: &Path) -> SourceDoc {
    let content = fs::read_to_string(path).expect("Failed to read source doc");
    let doc_name = path.file_stem().unwrap().to_string_lossy().to_string();

    let mut anchors = Vec::new();
    let mut sigla = Vec::new();
    let mut seen_sigla: HashMap<String, usize> = HashMap::new();
    let mut breaks = Vec::new();
    let mut work_links = Vec::new();
    let mut current_has_separator = false;
    let mut current_date: Option<i64> = None;
    let mut prev_date: Option<i64> = None;
    let mut in_remark = false;

    for line in content.lines() {
        if line.starts_with("### ") {
            if in_remark {
                // Determine break type for this new remark
                let brk = if current_has_separator {
                    RemarkBreak::Separator
                } else if let (Some(pd), Some(cd)) = (prev_date, current_date) {
                    if (cd - pd).abs() > 30 {
                        RemarkBreak::DateGap
                    } else {
                        RemarkBreak::None
                    }
                } else {
                    RemarkBreak::None
                };
                breaks.push(brk);
                prev_date = current_date;
            }
            anchors.push(anchor_from_doc_heading(line));
            work_links.push(work_links_of_heading(line));
            let siglum = format!("{doc_name},{}", fac_labels(line).join("et"));
            let n = seen_sigla.entry(siglum.clone()).or_default();
            *n += 1;
            sigla.push(if *n > 1 { format!("{siglum}-{n}") } else { siglum });
            current_has_separator = false;
            current_date = None;
            in_remark = true;
        } else if in_remark {
            if line.trim() == "---" {
                current_has_separator = true;
            }
            // Try to parse a date from the first non-empty content line
            if current_date.is_none() {
                if let Some(d) = parse_date(line) {
                    current_date = Some(d);
                }
            }
        }
    }

    SourceDoc {
        anchors,
        sigla,
        breaks,
        work_links,
    }
}

/// Build correspondences between work remarks and source doc remarks.
pub fn build_correspondence(
    work: &Work,
    source_docs: &HashMap<String, SourceDoc>,
) -> Vec<Correspondence> {
    let mut correspondences = Vec::new();

    // Build lookup: (doc_name, anchor) → index
    let mut anchor_map: HashMap<(&str, &str), usize> = HashMap::new();
    for (doc_name, doc) in source_docs {
        for (i, anchor) in doc.anchors.iter().enumerate() {
            anchor_map.insert((doc_name.as_str(), anchor.as_str()), i);
        }
    }

    for (work_idx, remark) in work.remarks.iter().enumerate() {
        if let Some(&source_idx) =
            anchor_map.get(&(remark.source_doc.as_str(), remark.source_anchor.as_str()))
        {
            correspondences.push(Correspondence {
                work_idx,
                doc_name: remark.source_doc.clone(),
                source_idx,
            });
        }
    }

    correspondences
}

/// Get the ordered list of unique source documents referenced by a work,
/// in the order they first appear.
pub fn source_doc_order(work: &Work) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut order = Vec::new();
    for remark in &work.remarks {
        if seen.insert(remark.source_doc.clone()) {
            order.push(remark.source_doc.clone());
        }
    }
    order
}

/// The work slugs a document's remarks link to, in order of first appearance.
pub fn doc_work_slugs(doc: &SourceDoc) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    doc.work_links
        .iter()
        .flatten()
        .filter(|(slug, _)| seen.insert(slug.clone()))
        .map(|(slug, _)| slug.clone())
        .collect()
}

/// Build correspondences for a document visualization, from the document's backlinks
/// (so a remark whose work page shows another document's copy still connects).
///
/// The returned correspondences have swapped semantics for use with `render_doc`:
/// - `work_idx` = document remark index (single column, right side)
/// - `doc_name` = work filename stem (multi column name, left side)
/// - `source_idx` = work remark index (multi column position, left side)
pub fn build_doc_correspondence(doc: &SourceDoc, works: &HashMap<String, Work>) -> Vec<Correspondence> {
    // (work slug, work page anchor) → (work stem, work remark index). The anchor of a
    // work remark is its source document and anchor there, as the parser writes it.
    let mut work_anchors: HashMap<(String, String), (&str, usize)> = HashMap::new();
    for (work_name, work) in works {
        let slug = work_name.to_lowercase();
        for (work_idx, remark) in work.remarks.iter().enumerate() {
            let anchor = format!(
                "{}-{}",
                remark.source_doc.to_lowercase(),
                remark.source_anchor
            );
            work_anchors
                .entry((slug.clone(), anchor))
                .or_insert((work_name.as_str(), work_idx));
        }
    }

    let mut all_corrs = Vec::new();
    for (doc_idx, links) in doc.work_links.iter().enumerate() {
        for link in links {
            if let Some(&(work_name, work_idx)) = work_anchors.get(link) {
                all_corrs.push(Correspondence {
                    work_idx: doc_idx,               // document remark index
                    doc_name: work_name.to_string(), // work name
                    source_idx: work_idx,            // work remark index
                });
            }
        }
    }
    all_corrs
}

/// Create a SourceDoc representation of a work (for layout in doc visualizations).
/// Uses 1-based remark numbers as anchors and no breaks.
pub fn work_as_source_doc(work: &Work) -> SourceDoc {
    let anchors: Vec<String> = (1..=work.remarks.len()).map(|i| i.to_string()).collect();
    SourceDoc {
        anchors,
        sigla: vec![],
        breaks: vec![],
        work_links: vec![],
    }
}

/// Load the pairs of similar remarks at or above `min_score` from similar/similar.tsv
/// (columns: a, b, score, shared, order), as (siglum, siglum) pairs.
pub fn load_similar(path: &Path, min_score: f64) -> Vec<(String, String)> {
    let content = fs::read_to_string(path).expect("Failed to read similar.tsv");
    content
        .lines()
        .skip(1)
        .filter_map(|line| {
            let cols: Vec<&str> = line.split('\t').collect();
            let score: f64 = cols.get(2)?.parse().ok()?;
            (score >= min_score).then(|| (cols[0].to_string(), cols[1].to_string()))
        })
        .collect()
}

/// The document a siglum belongs to (`Ms-115,118[2]` → `Ms-115`).
pub fn siglum_doc(siglum: &str) -> &str {
    siglum.split(',').next().unwrap_or(siglum)
}

/// The documents the site exposes: file stems linked from `all.md` in the markdown
/// directory. None (all documents) if all.md is missing.
pub fn published_docs(md_dir: &Path) -> Option<std::collections::HashSet<String>> {
    let Ok(content) = fs::read_to_string(md_dir.join("all.md")) else {
        eprintln!("Warning: all.md not found, including all documents");
        return None;
    };
    let re = Regex::new(r"\]\(([^)]+)\.md\)").unwrap();
    Some(re.captures_iter(&content).map(|c| c[1].to_string()).collect())
}
