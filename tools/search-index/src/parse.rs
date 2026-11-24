//! Extract one record per remark from the published markdown in `md/` and `md-en/`.
//!
//! The markdown is the contract (every other tool reads it too). A document file looks like:
//!
//! ```text
//! # Ms-101
//! <details> … viz … </details>
//! ### <span class="fac">[1r\[2\]](…/Ms-101/1r.webp),[2r\[1\]](…/Ms-101/2r.webp)</span>
//!
//! 10.08.1914
//!
//! Als Rekrut eingekleidet worden. …
//! ```
//!
//! English files (`md-en/`) carry byte-identical `### ` headings (verified), and the
//! bilingual page reuses the German heading, so the same fragment is correct for both
//! languages — English records only differ by the `/en/` URL prefix and the content.

use crate::record::WorkLink;
use crate::slug;
use regex::Regex;

/// Compiled regexes, built once and reused across every remark.
pub struct Res {
    fac_span: Regex,
    link: Regex,
    work_link: Regex,
    date: Regex,
    series: Regex,
    math_inline: Regex,
    em: Regex,
    intraword_pre: Regex,
    intraword_post: Regex,
    md_link: Regex,
    variant: Regex,
    strike: Regex,
    tag: Regex,
    ws: Regex,
}

impl Res {
    pub fn new() -> Self {
        Res {
            // `<span class="fac"> … </span>` — the facsimile references in a heading.
            fac_span: Regex::new(r#"<span class="fac">(.*?)</span>"#).unwrap(),
            // A markdown link `[label](url)` inside the fac span. Labels contain escaped
            // brackets (`1r\[2\]`); the real closing `]` is the one followed by `(`, which
            // the non-greedy `.*?` finds correctly.
            link: Regex::new(r"\[(.*?)\]\(([^)]*)\)").unwrap(),
            // A work link in a document heading: `[RFM III](/w-rfm-3/#ms-122-5r.2+5v.1)`.
            // Captures the display label and the work URL (which starts with `/w-`).
            work_link: Regex::new(r"\[([^\]]+)\]\((/w-[^)]+)\)").unwrap(),
            // A standalone written-date line, `DD.MM.YYYY`.
            date: Regex::new(r"^(\d{2})\.(\d{2})\.(\d{4})$").unwrap(),
            // Published series number, e.g. `<span class="series-number">1˙1</span>`.
            series: Regex::new(r#"<span class="series-number">(.*?)</span>"#).unwrap(),
            // Inline math: legacy MathML or a rendered LaTeX formula image.
            math_inline: Regex::new(r#"(?s)<math\b.*?</math>|<img class="math[^"]*"[^>]*>"#).unwrap(),
            // Markdown emphasis `_word_` (Wittgenstein's underlinings).
            em: Regex::new(r"_([^_]+)_").unwrap(),
            // Emphasis inside a word, `Empfindungs*begriff*` / `*Bewegungs*möglichkeit`
            // (the parser writes it with `*`, since `_` doesn't work inside a word).
            intraword_pre: Regex::new(r"(\w)\*([^*\s]+)\*").unwrap(),
            intraword_post: Regex::new(r"\*([^*\s]+)\*(\w)").unwrap(),
            // Markdown link `[text](url)` -> keep the text.
            md_link: Regex::new(r"\[([^\]]*)\]\([^)]*\)").unwrap(),
            // `{primary // alternative}` reading variants -> keep the primary.
            variant: Regex::new(r"\{([^/}]*)\s*//[^}]*\}").unwrap(),
            // `<s …>deleted</s>` strikethrough -> drop entirely (editorially deleted text).
            // `<s\b` does not match `<sup>` (no word boundary between `s` and `u`).
            strike: Regex::new(r"(?s)<s\b[^>]*>.*?</s>").unwrap(),
            // Any remaining HTML tag.
            tag: Regex::new(r"<[^>]+>").unwrap(),
            // Whitespace run.
            ws: Regex::new(r"\s+").unwrap(),
        }
    }
}

pub struct DocMeta {
    pub doc: String,
    pub doc_slug: String,
    pub doctype: String,
}

pub struct Remark {
    pub page_refs: Vec<String>,
    pub fragment: String,
    pub image_url: Option<String>,
    pub date: Option<String>,
    pub date_sort: Option<u64>,
    pub series_number: Option<String>,
    pub content: String,
    pub works: Vec<WorkLink>,
}

/// Read `# Title` from the preamble and derive the document name, slug and type.
fn doc_meta(content: &str) -> Option<DocMeta> {
    let title = content
        .lines()
        .find(|l| l.starts_with("# "))
        .map(|l| l[2..].trim().to_string())?;
    let doctype = title.split('-').next().unwrap_or("").to_string();
    Some(DocMeta {
        doc: title.clone(),
        doc_slug: title.to_lowercase(),
        doctype,
    })
}

/// Split a file into its `### ` remark blocks. Returns each block's heading line and the
/// lines of its body (everything up to the next `### `). The preamble before the first
/// heading (title + `<details>` viz) is ignored.
fn remark_blocks(content: &str) -> Vec<(String, Vec<&str>)> {
    let mut blocks: Vec<(String, Vec<&str>)> = Vec::new();
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("### ") {
            blocks.push((rest.to_string(), Vec::new()));
        } else if let Some(last) = blocks.last_mut() {
            last.1.push(line);
        }
    }
    blocks
}

/// Pull the linked facsimile labels, their image URLs and the heading id out of a
/// heading's `<span class="fac">`. Returns `(page_refs, image_urls, fragment)` with
/// brackets unescaped for display.
fn parse_fac(heading: &str, res: &Res) -> (Vec<String>, Vec<String>, String) {
    let mut refs = Vec::new();
    let mut urls = Vec::new();
    let mut fragment = String::new();
    if let Some(span) = res.fac_span.captures(heading) {
        for cap in res.link.captures_iter(&span[1]) {
            refs.push(slug::unescape_label(&cap[1]));
            urls.push(cap[2].to_string());
        }
        fragment = slug::remark_id(&span[1]);
    }
    (refs, urls, fragment)
}

fn is_horizontal_rule(line: &str) -> bool {
    let chars: Vec<char> = line.chars().filter(|c| !c.is_whitespace()).collect();
    chars.len() >= 3 && chars.iter().all(|&c| c == '-' || c == '*' || c == '_')
}

fn parse_date(line: &str, res: &Res) -> Option<(String, u64)> {
    let c = res.date.captures(line)?;
    let (d, m, y) = (&c[1], &c[2], &c[3]);
    let sort: u64 = format!("{y}{m}{d}").parse().ok()?;
    Some((format!("{y}-{m}-{d}"), sort))
}

/// Strip markdown/HTML to plain searchable text (adapted from `tools/covers/parse.rs`).
fn strip_markup(text: &str, res: &Res) -> String {
    let mut s = text.to_string();
    s = res.strike.replace_all(&s, "").to_string();
    s = res.math_inline.replace_all(&s, "").to_string();
    s = res.tag.replace_all(&s, "").to_string();
    s = s.replace("**", "");
    s = res.intraword_pre.replace_all(&s, "$1$2").to_string();
    s = res.intraword_post.replace_all(&s, "$1$2").to_string();
    s = res.em.replace_all(&s, "$1").to_string();
    s = res.md_link.replace_all(&s, "$1").to_string();
    s = res.variant.replace_all(&s, "$1").to_string();
    s
}

/// Extract the optional date, optional series number, and plain-text content from a body.
fn parse_body(lines: &[&str], res: &Res) -> (Option<String>, Option<u64>, Option<String>, String) {
    let mut date = None;
    let mut date_sort = None;
    let mut series = None;
    let mut in_math_block = false;
    let mut buf = String::new();

    for &raw in lines {
        let t = raw.trim();

        // Skip multi-line `<math display="block"> … </math>`.
        if in_math_block {
            if t.contains("</math>") {
                in_math_block = false;
            }
            continue;
        }
        if t.contains("<math") && t.contains(r#"display="block""#) && !t.contains("</math>") {
            in_math_block = true;
            continue;
        }

        if t.is_empty() || t.starts_with('#') || is_horizontal_rule(t) {
            continue;
        }
        // Standalone graphics/image lines carry no searchable prose.
        if t.starts_with("![") || t.starts_with("<img") {
            continue;
        }
        // The written date appears on its own line, right after the heading.
        if date.is_none() {
            if let Some((iso, sort)) = parse_date(t, res) {
                date = Some(iso);
                date_sort = Some(sort);
                continue;
            }
        }

        let mut s = raw.to_string();
        // Capture, then remove, the series-number span so its digits don't leak into content.
        if series.is_none() {
            if let Some(c) = res.series.captures(&s) {
                series = Some(c[1].trim().to_string());
            }
        }
        s = res.series.replace_all(&s, "").to_string();

        let stripped = strip_markup(&s, res);
        let stripped = stripped.trim();
        if stripped.is_empty() {
            continue;
        }
        if !buf.is_empty() {
            buf.push(' ');
        }
        buf.push_str(stripped);
    }

    let content = res.ws.replace_all(&buf, " ").trim().to_string();
    (date, date_sort, series, content)
}

/// Parse one document or English file into its remarks. Returns `None` for files with no
/// `# Title` heading. Remarks with no facsimile reference (and thus no anchor) are skipped.
pub fn parse_file(content: &str, res: &Res) -> Option<(DocMeta, Vec<Remark>)> {
    let meta = doc_meta(content)?;
    let mut remarks = Vec::new();

    for (heading, body) in remark_blocks(content) {
        // The heading id comes from the site's render hook, which never de-duplicates
        // (the same id can appear twice on a page if a document repeats a ref).
        let (page_refs, urls, fragment) = parse_fac(&heading, res);
        if page_refs.is_empty() {
            continue;
        }

        let (date, date_sort, series_number, content) = parse_body(&body, res);
        remarks.push(Remark {
            page_refs,
            fragment,
            image_url: urls.into_iter().next(),
            date,
            date_sort,
            series_number,
            content,
            works: parse_works(&heading, res),
        });
    }

    Some((meta, remarks))
}

/// Extract the works a remark is published in straight from its document heading, e.g.
/// `### [RFM III](/w-rfm-3/#ms-122-5r.2+5v.1) <span class="fac">…</span>`. This gives exactly
/// the label and link the document pages show. The URL is rewritten exactly like the
/// `single.html`/`bilingual.html` templates rewrite internal links, so it matches the work
/// page's canonical heading id: lowercase, `.` → `-`, `+` → `--`
/// (`#ms-122-5r.2+5v.1` → `#ms-122-5r-2--5v-1`).
fn parse_works(heading: &str, res: &Res) -> Vec<WorkLink> {
    let mut out = Vec::new();
    for cap in res.work_link.captures_iter(heading) {
        let label = cap[1].to_string();
        let url = cap[2].replace('.', "-").replace('+', "--").to_lowercase();
        // Work-level code (for filtering) from the part slug, e.g. "/w-rfm-3/…" -> "W-RFM".
        let slug = url.trim_start_matches('/').split('/').next().unwrap_or("");
        let code = work_code(&slug.to_uppercase());
        out.push(WorkLink { code, label, url });
    }
    out
}

/// The base work code for a Work-file stem: the leading `W-<letters>`, dropping any part /
/// appendix suffix. `W-RFM-3` -> `W-RFM`, `W-PG-1-App-4` -> `W-PG`, `W-PI` -> `W-PI`.
fn work_code(stem: &str) -> String {
    let base = Regex::new(r"^(W-[A-Za-z]+)").unwrap();
    match base.captures(stem) {
        Some(c) => c[1].to_string(),
        None => stem.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = r#"# Ms-101

<details>
<summary>187 published remarks</summary>
<img class="viz" src="/viz/Ms-101.svg">
</details>

### <span class="fac">[1r\[2\]](https://cdn/2000px/webp/Ms-101/1r.webp),[2r\[1\]](https://cdn/2000px/webp/Ms-101/2r.webp)</span>

10.08.1914

Als Rekrut _eingekleidet_ worden. Wenig **Hoffnung** zu können.

### <span class="fac">[IIr\[1\]](https://cdn/2000px/webp/Ms-101/IIr.webp)</span>

Nach meinem Tod zu senden
"#;

    #[test]
    fn extracts_remarks_with_correct_fragments() {
        let res = Res::new();
        let (meta, remarks) = parse_file(DOC, &res).unwrap();
        assert_eq!(meta.doc, "Ms-101");
        assert_eq!(meta.doc_slug, "ms-101");
        assert_eq!(meta.doctype, "Ms");
        assert_eq!(remarks.len(), 2);

        let r0 = &remarks[0];
        assert_eq!(r0.fragment, "1r-2--2r-1");
        assert_eq!(r0.page_refs, vec!["1r[2]", "2r[1]"]);
        assert_eq!(r0.image_url.as_deref(), Some("https://cdn/2000px/webp/Ms-101/1r.webp"));
        assert_eq!(r0.date.as_deref(), Some("1914-08-10"));
        assert_eq!(r0.date_sort, Some(19140810));
        // Emphasis and bold markers are stripped; the date line is not in the content.
        assert_eq!(r0.content, "Als Rekrut eingekleidet worden. Wenig Hoffnung zu können.");

        assert_eq!(remarks[1].fragment, "iir-1");
        assert_eq!(remarks[1].content, "Nach meinem Tod zu senden");
    }

    #[test]
    fn series_number_is_captured_and_removed() {
        let res = Res::new();
        let doc = "# Ms-104\n\n### <span class=\"fac\">[1\\[1\\]](https://cdn/webp/Ms-104/1.webp)</span>\n\n<span class=\"series-number\">1˙1</span> Die Welt ist die Gesamtheit der Tatsachen.\n";
        let (_, remarks) = parse_file(doc, &res).unwrap();
        assert_eq!(remarks[0].series_number.as_deref(), Some("1˙1"));
        assert_eq!(remarks[0].content, "Die Welt ist die Gesamtheit der Tatsachen.");
    }

    #[test]
    fn fragments_keep_ref_boundaries() {
        // 3[12] and 31[2] would both be "312" under Goldmark's auto-ids.
        let res = Res::new();
        let doc = "# Ms-104\n\n### [PT](/w-pt/#ms-104-3.12) <span class=\"fac\">[3\\[12\\]](u/Ms-104/3.webp)</span>\n\na\n\n### [PT](/w-pt/#ms-104-31.2) <span class=\"fac\">[31\\[2\\]](u/Ms-104/31.webp)</span>\n\nb\n";
        let (_, remarks) = parse_file(doc, &res).unwrap();
        assert_eq!(remarks[0].fragment, "3-12");
        assert_eq!(remarks[1].fragment, "31-2");
    }

    #[test]
    fn parse_works_uses_the_doc_heading_label_and_part_link() {
        let res = Res::new();
        // A document heading for a remark published in RFM part III.
        let heading = r#"[RFM III](/w-rfm-3/#ms-122-5r.2+5v.1) <span class="fac">[5r\[2\]](u/Ms-122/5r.webp)</span>"#;
        let works = parse_works(heading, &res);
        assert_eq!(works.len(), 1);
        assert_eq!(works[0].label, "RFM III"); // exactly as the doc page shows it
        assert_eq!(works[0].code, "W-RFM"); // collapsed work code, for filtering
        assert_eq!(works[0].url, "/w-rfm-3/#ms-122-5r-2--5v-1"); // part page, canonical heading id
    }

    #[test]
    fn parse_works_empty_for_unpublished_heading() {
        let res = Res::new();
        let heading = r#"<span class="fac">[1r\[1\]](u/Ms-101/1r.webp)</span>"#;
        assert!(parse_works(heading, &res).is_empty());
    }

    #[test]
    fn work_code_collapses_parts() {
        assert_eq!(work_code("W-RFM-7"), "W-RFM");
        assert_eq!(work_code("W-PG-1-App-4"), "W-PG");
        assert_eq!(work_code("W-PI"), "W-PI");
        assert_eq!(work_code("W-OC"), "W-OC");
    }
}
