use regex::Regex;

pub struct PreparedMarkdown {
    pub title: String,
    pub content: String,
}

/// Check if a markdown file is an index file (has bullet-point links to parts).
pub fn is_index_file(raw: &str) -> bool {
    let mut past_details = false;
    let mut in_details = false;
    for line in raw.lines() {
        if line.contains("<details") {
            in_details = true;
        }
        if line.contains("</details>") {
            in_details = false;
            past_details = true;
            continue;
        }
        if in_details {
            continue;
        }
        if past_details && line.starts_with("- [") && line.contains("](/") {
            return true;
        }
    }
    false
}

/// Parse part slugs from an index file's bullet-point links.
/// Returns slugs like "w-rfm-1", "w-rfm-1-app-1", etc.
pub fn parse_index_parts(raw: &str) -> Vec<String> {
    let mut slugs = Vec::new();
    let re = Regex::new(r"^- \[.*\]\(/([^/]+)/\)").unwrap();
    for line in raw.lines() {
        if let Some(cap) = re.captures(line) {
            slugs.push(cap[1].to_string());
        }
    }
    slugs
}

/// Prepare a multi-part book from an index file and its part contents.
pub fn prepare_book(index_raw: &str, part_raws: &[String], author: &str) -> PreparedMarkdown {
    let link_re = Regex::new(r#"\[([^\]\\]*(?:\\.[^\]\\]*)*)\]\(([^)]+)\)"#).unwrap();

    // Extract title from index file
    let mut title = String::new();
    for line in index_raw.lines() {
        if line.starts_with("# ") {
            title = line[2..].trim().to_string();
            break;
        }
    }

    let mut content = String::new();
    content.push_str("---\n");
    content.push_str(&format!("title: \"{}\"\n", yaml_escape(&title)));
    content.push_str(&format!("author: \"{}\"\n", yaml_escape(author)));
    content.push_str("lang: de\n");
    content.push_str("---\n\n");

    for part_raw in part_raws {
        let (part_title, body) = extract_part(&link_re, part_raw);

        // Use the part suffix as chapter heading (after " – "), or the full title
        let chapter_heading = if let Some(pos) = part_title.find(" – ") {
            part_title[pos + " – ".len()..].to_string()
        } else {
            part_title
        };

        if !chapter_heading.is_empty() {
            content.push_str(&format!("## {}\n\n", chapter_heading));
        }
        content.push_str(&body);
        content.push_str("\n\n");
    }

    PreparedMarkdown { title, content }
}

/// Prepare a single markdown file for PDF conversion via pandoc + weasyprint.
pub fn prepare(raw: &str, author: &str) -> PreparedMarkdown {
    let link_re = Regex::new(r#"\[([^\]\\]*(?:\\.[^\]\\]*)*)\]\(([^)]+)\)"#).unwrap();

    let mut title = String::new();
    let mut body_lines: Vec<String> = Vec::new();
    let mut found_title = false;
    let mut in_details = false;

    for line in raw.lines() {
        if !found_title && line.starts_with("# ") {
            title = line[2..].trim().to_string();
            found_title = true;
            continue;
        }

        if line.contains("<details") {
            in_details = true;
            continue;
        }
        if line.contains("</details>") {
            in_details = false;
            continue;
        }
        if in_details {
            continue;
        }

        if let Some(html) = convert_h3(&link_re, line) {
            body_lines.push(html);
            continue;
        }

        // Rewrite graphics paths to use bbox-adjusted epub versions
        body_lines.push(
            line.replace("../graphics/", "../graphics-cropped/")
                .to_string(),
        );
    }

    // Remove leading empty lines
    while body_lines.first().map_or(false, |l| l.trim().is_empty()) {
        body_lines.remove(0);
    }

    // Wrap h3 + first content line to keep them on the same page
    let body_lines = wrap_h3_with_next(&body_lines);

    let mut content = String::new();
    content.push_str("---\n");
    content.push_str(&format!("title: \"{}\"\n", yaml_escape(&title)));
    content.push_str(&format!("author: \"{}\"\n", yaml_escape(author)));
    content.push_str("lang: de\n");
    content.push_str("---\n\n");

    for line in &body_lines {
        content.push_str(line);
        content.push('\n');
    }

    PreparedMarkdown { title, content }
}

/// Extract title and body from a part file (for book assembly).
fn extract_part(link_re: &Regex, raw: &str) -> (String, String) {
    let mut title = String::new();
    let mut body_lines: Vec<String> = Vec::new();
    let mut found_title = false;
    let mut in_details = false;

    for line in raw.lines() {
        if !found_title && line.starts_with("# ") {
            title = line[2..].trim().to_string();
            found_title = true;
            continue;
        }

        if line.contains("<details") {
            in_details = true;
            continue;
        }
        if line.contains("</details>") {
            in_details = false;
            continue;
        }
        if in_details {
            continue;
        }

        if let Some(html) = convert_h3(link_re, line) {
            body_lines.push(html);
            continue;
        }

        body_lines.push(
            line.replace("../graphics/", "../graphics-cropped/")
                .to_string(),
        );
    }

    // Remove leading empty lines
    while body_lines.first().map_or(false, |l| l.trim().is_empty()) {
        body_lines.remove(0);
    }

    let body_lines = wrap_h3_with_next(&body_lines);
    let body = body_lines.join("\n");
    (title, body)
}

/// Convert an h3 markdown line to raw HTML with <br> between multiple links.
/// Handles two formats:
///   Ms-xxx:  ### [IIr\[1\]](url),[IIr\[2\]](url)       → "IIr[1] &\n IIr[2]"
///   W-xxx:   ### [Ts-222](ref): [1\[1\]](url),[2\[1\]](url) → "Ts-222: 1[1] &\n 2[1]"
/// Wrap each <h3> and the first following non-empty line in a
/// <div class="remark"> so they stay on the same page.
fn wrap_h3_with_next(lines: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if lines[i].starts_with("<h3>") {
            out.push("<div class=\"remark\">".to_string());
            out.push(lines[i].clone());
            out.push(String::new());
            // Include lines until the first non-empty content line after the h3
            i += 1;
            // Skip blank lines
            while i < lines.len() && lines[i].trim().is_empty() {
                out.push(lines[i].clone());
                i += 1;
            }
            // Include the first non-empty line (the start of the remark text)
            if i < lines.len() {
                out.push(lines[i].clone());
                i += 1;
            }
            out.push(String::new());
            out.push("</div>".to_string());
            out.push(String::new());
        } else {
            out.push(lines[i].clone());
            i += 1;
        }
    }
    out
}

/// Extract markdown links `[text](url)` from `s`, unescaping `\[`/`\]` in the
/// link text.
fn collect_links(link_re: &Regex, s: &str) -> Vec<(String, String)> {
    link_re
        .captures_iter(s)
        .map(|cap| {
            let text = cap[1].replace("\\[", "[").replace("\\]", "]");
            let url = cap[2].to_string();
            (text, url)
        })
        .collect()
}

/// Render facsimile links into `html`, joined by a line break and an ampersand
/// (so multiple pages read "21[5] & 22[1]").
fn push_fac_links(html: &mut String, fac_links: &[(String, String)]) {
    for (i, (text, url)) in fac_links.iter().enumerate() {
        if i > 0 {
            html.push_str("<br>");
        }
        html.push_str(&format!("<a href=\"{}\">{}</a>", url, text));
        if i < fac_links.len() - 1 {
            html.push_str("&nbsp;&amp;");
        }
    }
}

fn convert_h3(link_re: &Regex, line: &str) -> Option<String> {
    if !line.starts_with("### ") {
        return None;
    }
    let rest = &line[4..];

    // Current format: optional work/doc links, then a facsimile span:
    //   ### [Ms-172](…) <span class="fac">[21\[1\]](…),[22\[1\]](…)</span> {#…}
    // The work/doc links precede the span; the comma-joined facsimile links sit
    // inside it. The trailing "{#…}" heading id (after </span>) is ignored, as
    // the PDF has no use for these anchors.
    const SPAN_OPEN: &str = "<span class=\"fac\">";
    if let Some(span_start) = rest.find(SPAN_OPEN) {
        let prefix = &rest[..span_start];
        let after = &rest[span_start + SPAN_OPEN.len()..];
        let fac_part = match after.find("</span>") {
            Some(end) => &after[..end],
            None => after,
        };

        let prefix_links = collect_links(link_re, prefix);
        let fac_links = collect_links(link_re, fac_part);

        if prefix_links.is_empty() && fac_links.is_empty() {
            return None;
        }

        let mut html = String::from("<h3>");
        // Work/doc links, comma-separated (matching the source markdown).
        for (i, (text, url)) in prefix_links.iter().enumerate() {
            if i > 0 {
                html.push_str(", ");
            }
            html.push_str(&format!("<a href=\"{}\">{}</a>", url, text));
        }
        // The doc/work title gets its own line above the facsimile pages (a
        // line break, never an ampersand — ampersands only join pages).
        if !prefix_links.is_empty() && !fac_links.is_empty() {
            html.push_str("<br>");
        }
        push_fac_links(&mut html, &fac_links);
        html.push_str("</h3>");
        return Some(html);
    }

    // Fallback: a heading that is just facsimile links, no span.
    let links = collect_links(link_re, rest);
    if links.is_empty() {
        return None;
    }

    let mut html = String::from("<h3>");
    push_fac_links(&mut html, &links);
    html.push_str("</h3>");
    Some(html)
}

fn yaml_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link_re() -> Regex {
        Regex::new(r#"\[([^\]\\]*(?:\\.[^\]\\]*)*)\]\(([^)]+)\)"#).unwrap()
    }

    #[test]
    fn doc_link_and_single_facsimile_have_no_ampersand() {
        let re = link_re();
        let line = r#"### [Ms-172](/ms-172/#21.1) <span class="fac">[21\[1\]](https://cdn/Ms-172/21.webp)</span> {#ms-172-211}"#;
        let html = convert_h3(&re, line).unwrap();
        assert_eq!(
            html,
            r#"<h3><a href="/ms-172/#21.1">Ms-172</a><br><a href="https://cdn/Ms-172/21.webp">21[1]</a></h3>"#
        );
        assert!(!html.contains("&amp;"));
    }

    #[test]
    fn multiple_facsimiles_are_joined_by_ampersand() {
        let re = link_re();
        let line = r#"### [Ms-172](/ms-172/#21.5+22.1) <span class="fac">[21\[5\]](https://cdn/Ms-172/21.webp),[22\[1\]](https://cdn/Ms-172/22.webp)</span> {#ms-172-215221}"#;
        let html = convert_h3(&re, line).unwrap();
        // Exactly one ampersand, between the two facsimile pages.
        assert_eq!(html.matches("&amp;").count(), 1);
        assert!(html.contains(r#">21[5]</a>&nbsp;&amp;<br><a "#));
    }

    #[test]
    fn facsimile_only_heading_has_no_prefix() {
        let re = link_re();
        let line = r#"### <span class="fac">[1\[1\]](https://cdn/Ms-172/1.webp)</span> {#11}"#;
        let html = convert_h3(&re, line).unwrap();
        assert_eq!(
            html,
            r#"<h3><a href="https://cdn/Ms-172/1.webp">1[1]</a></h3>"#
        );
    }
}
