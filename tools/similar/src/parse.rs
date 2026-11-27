use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::LazyLock;

pub struct Remark {
    /// Bergen-style siglum, e.g. `Ms-115,118[2]et119[1]`, unique within its document.
    pub siglum: String,
    /// Key into `md/dates.tsv`: the siglum without the `.N` suffix of split remarks.
    pub date_key: String,
    /// Raw Markdown body (everything between this heading and the next).
    pub body: String,
}

static FAC_SPAN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"<span class="fac">(.*?)</span>"#).unwrap());

/// Build the siglum part after the comma (`118[2]et119[1]`) from a document remark heading:
/// every facsimile label in its `fac` span, linked or not (labels without a facsimile are
/// plain text: `XVIII-134-1\[1\],[XVIII-134-2\[1\]](url)`), as the site derives the
/// heading id from them (site/layouts/_default/_markup/render-heading.html).
pub fn page_refs(heading: &str) -> String {
    let Some(fac) = FAC_SPAN.captures(heading) else {
        return String::new();
    };
    fac[1]
        .split(',')
        .map(|label| {
            // `[label](url)` → `label`; plain-text labels pass through.
            let label = match (label.strip_prefix('['), label.find("](")) {
                (Some(_), Some(end)) => &label[1..end],
                _ => label,
            };
            label.replace("\\[", "[").replace("\\]", "]")
        })
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
                date_key: date_base(&siglum).to_string(),
                siglum,
                body: String::new(),
            });
        } else if let Some(r) = remarks.last_mut() {
            r.body.push_str(line);
            r.body.push('\n');
        }
    }

    // The parser guarantees unique sigla; a duplicate would conflate two remarks here
    // and in tools/add_versions.py and tools/visualize, which key remarks the same way.
    let mut seen: HashSet<&str> = HashSet::new();
    for r in &remarks {
        assert!(seen.insert(&r.siglum), "{doc}: siglum occurs twice: {}", r.siglum);
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

/// Load `dates.tsv` (written by the parser, one row per remark of the XML): siglum → date
/// range, if the remark is dated.
pub fn load_dates(path: &Path) -> HashMap<String, Option<(String, String)>> {
    let content = fs::read_to_string(path).expect("cannot read dates.tsv (run the parser first)");
    let mut dates = HashMap::new();
    for line in content.lines().skip(1) {
        let mut cols = line.split('\t');
        let (Some(siglum), Some(start), Some(end)) = (cols.next(), cols.next(), cols.next()) else {
            continue;
        };
        let range = (!start.is_empty()).then(|| (start.to_string(), end.to_string()));
        dates.insert(siglum.to_string(), range);
    }
    dates
}

#[cfg(test)]
mod tests {
    use super::{date_base, page_refs};

    #[test]
    fn reads_linked_and_unlinked_labels() {
        let h = |fac: &str| format!(r#"<span class="fac">{fac}</span>"#);
        assert_eq!(page_refs(&h(r"[1r\[2\]](https://cdn/1r.webp)")), "1r[2]");
        assert_eq!(
            page_refs(&h(r"XVIII-134-3\[1\],[XVIII-134-4\[1\]](https://cdn/4.webp)")),
            "XVIII-134-3[1]etXVIII-134-4[1]"
        );
        assert_eq!(
            page_refs(&h(r"[XII-89-33\[1\]](https://cdn/33.webp),XII-89-34\[1\]")),
            "XII-89-33[1]etXII-89-34[1]"
        );
        assert_eq!(page_refs(&h(r"I-1-9v\[1\]")), "I-1-9v[1]");
        let work = r#"[OC](/w-oc/#ms-175-54v.2+55r.1) <span class="fac">[54v\[2\]](https://cdn/54v.webp)</span>"#;
        assert_eq!(page_refs(work), "54v[2]");
    }

    #[test]
    fn strips_split_suffix_only() {
        assert_eq!(date_base("Ts-233b,14[4]et15[1].2"), "Ts-233b,14[4]et15[1]");
        assert_eq!(date_base("Ms-115,1[2]"), "Ms-115,1[2]");
        assert_eq!(date_base("Ts-202,41av[2]-relocated"), "Ts-202,41av[2]-relocated");
    }
}
