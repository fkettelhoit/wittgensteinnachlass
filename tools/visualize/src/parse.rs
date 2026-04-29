use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub struct WorkRemark {
    pub source_doc: String,
    pub source_anchor: String,
}

pub struct Work {
    pub filename: String,
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
    /// For each remark (except the first), what kind of break precedes it.
    pub breaks: Vec<RemarkBreak>,
}

pub struct Correspondence {
    pub work_idx: usize,
    pub doc_name: String,
    pub source_idx: usize,
}

/// Compute the anchor ID from a remark heading's page refs.
/// E.g., heading `### [1\[1\]](url)` → page ref text `1[1]` → anchor `1.1`
fn anchor_from_doc_heading(heading: &str) -> String {
    // Extract the link text portions (page refs) from the heading
    let link_re = Regex::new(r"\[([^\]]*(?:\\.[^\]]*)*)\]\([^)]+\)").unwrap();
    let mut parts = Vec::new();
    for cap in link_re.captures_iter(heading) {
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

/// Parse a work markdown file to extract source references.
pub fn parse_work(path: &Path) -> Work {
    let content = fs::read_to_string(path).expect("Failed to read work file");
    let filename = path.file_name().unwrap().to_string_lossy().to_string();

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

    Work { filename, remarks }
}

/// Parse a DD.MM.YYYY date string into a day count for comparison.
fn parse_date(s: &str) -> Option<i64> {
    let date_re = Regex::new(r"^(\d{2})\.(\d{2})\.(\d{4})$").unwrap();
    let cap = date_re.captures(s.trim())?;
    let d: i64 = cap[1].parse().ok()?;
    let m: i64 = cap[2].parse().ok()?;
    let y: i64 = cap[3].parse().ok()?;
    // Rough day count — exact accuracy not needed, just >30 day detection
    Some(y * 365 + m * 30 + d)
}

/// Parse a source document to get remark anchors and inter-remark breaks.
pub fn parse_source_doc(path: &Path) -> SourceDoc {
    let content = fs::read_to_string(path).expect("Failed to read source doc");

    let mut anchors = Vec::new();
    let mut breaks = Vec::new();
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

    SourceDoc { anchors, breaks }
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
