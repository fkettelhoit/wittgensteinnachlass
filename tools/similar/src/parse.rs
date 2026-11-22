use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::LazyLock;

pub struct Remark {
    /// Bergen-style siglum, e.g. `Ms-115,118[2]et119[1]`, made unique within its document.
    pub siglum: String,
    /// Key into `md/dates.tsv`: the siglum without the `.N` suffix of split remarks and
    /// without our `-N` disambiguation, plus which occurrence of it this is (0-based).
    pub date_key: (String, usize),
    /// Raw Markdown body (everything between this heading and the next).
    pub body: String,
}

static FAC_SPAN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"<span class="fac">(.*?)</span>"#).unwrap());
/// A Markdown link whose text may contain escaped brackets (`1\[2\]`) and whose URL is external.
static FAC_LINK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[((?:\\.|[^\]\\])*)\]\((https?://[^)]+)\)").unwrap());

/// Build the siglum part after the comma (`118[2]et119[1]`) from a document remark heading.
fn page_refs(heading: &str) -> String {
    let src = FAC_SPAN
        .captures(heading)
        .map(|c| c.get(1).unwrap().as_str())
        .unwrap_or(heading);
    FAC_LINK
        .captures_iter(src)
        .map(|c| c[1].replace("\\[", "[").replace("\\]", "]"))
        .collect::<Vec<_>>()
        .join("et")
}

/// Split a document file into remarks. Content before the first `### ` heading
/// (title, metadata block) is ignored.
pub fn parse_doc(path: &Path) -> Vec<Remark> {
    let doc = path.file_stem().unwrap().to_string_lossy().to_string();
    let content = fs::read_to_string(path).expect("failed to read document");

    let mut remarks: Vec<Remark> = Vec::new();
    for line in content.lines() {
        if let Some(heading) = line.strip_prefix("### ") {
            let refs = page_refs(heading);
            if refs.is_empty() {
                eprintln!("warning: {doc}: no page reference in heading: {line}");
            }
            let siglum = format!("{doc},{refs}");
            remarks.push(Remark {
                date_key: (date_base(&siglum).to_string(), 0),
                siglum,
                body: String::new(),
            });
        } else if let Some(r) = remarks.last_mut() {
            r.body.push_str(line);
            r.body.push('\n');
        }
    }

    // Disambiguate repeated sigla (cut-up typescripts): 2nd occurrence gets `-2`, etc.
    let mut seen: HashMap<String, usize> = HashMap::new();
    for r in &mut remarks {
        let n = seen.entry(r.siglum.clone()).or_insert(0);
        *n += 1;
        r.date_key.1 = *n - 1;
        if *n > 1 {
            r.siglum = format!("{}-{}", r.siglum, n);
        }
    }
    remarks
}

/// Strip the `.N` suffix the parser adds to remarks split across work entries
/// (`Ts-233b,14[4]et15[1].2` → `Ts-233b,14[4]et15[1]`): they share the XML remark's date.
fn date_base(siglum: &str) -> &str {
    match siglum.rfind("].") {
        Some(i) if siglum[i + 2..].chars().all(|c| c.is_ascii_digit()) && i + 2 < siglum.len() => {
            &siglum[..=i]
        }
        _ => siglum,
    }
}

/// Load `dates.tsv` (written by the parser): siglum → date ranges in document order
/// (a siglum can occur more than once in cut-up typescripts).
pub fn load_dates(path: &Path) -> HashMap<String, Vec<Option<(String, String)>>> {
    let content = fs::read_to_string(path).expect("cannot read dates.tsv (run the parser first)");
    let mut dates: HashMap<String, Vec<Option<(String, String)>>> = HashMap::new();
    for line in content.lines().skip(1) {
        let mut cols = line.split('\t');
        let (Some(siglum), Some(start), Some(end)) = (cols.next(), cols.next(), cols.next()) else {
            continue;
        };
        let range = (!start.is_empty()).then(|| (start.to_string(), end.to_string()));
        dates.entry(siglum.to_string()).or_default().push(range);
    }
    dates
}

#[cfg(test)]
mod tests {
    use super::date_base;

    #[test]
    fn strips_split_suffix_only() {
        assert_eq!(date_base("Ts-233b,14[4]et15[1].2"), "Ts-233b,14[4]et15[1]");
        assert_eq!(date_base("Ms-115,1[2]"), "Ms-115,1[2]");
        assert_eq!(date_base("Ts-202,41av[2]-relocated"), "Ts-202,41av[2]-relocated");
    }
}
