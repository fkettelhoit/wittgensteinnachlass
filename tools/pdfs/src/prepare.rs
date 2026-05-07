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

fn convert_h3(link_re: &Regex, line: &str) -> Option<String> {
    if !line.starts_with("### ") {
        return None;
    }
    let rest = &line[4..];

    // Check if there's a "): " separator (source ref : facsimile links)
    if let Some(colon_pos) = rest.find("): ") {
        let source_part = &rest[..colon_pos + 1]; // include the closing )
        let fac_part = &rest[colon_pos + 3..]; // skip "): "

        let source_links: Vec<(String, String)> = link_re
            .captures_iter(source_part)
            .map(|cap| {
                let text = cap[1].replace("\\[", "[").replace("\\]", "]");
                let url = cap[2].to_string();
                (text, url)
            })
            .collect();

        let fac_links: Vec<(String, String)> = link_re
            .captures_iter(fac_part)
            .map(|cap| {
                let text = cap[1].replace("\\[", "[").replace("\\]", "]");
                let url = cap[2].to_string();
                (text, url)
            })
            .collect();

        if source_links.is_empty() && fac_links.is_empty() {
            return None;
        }

        let mut html = String::from("<h3>");
        // Source ref with colon
        for (text, url) in &source_links {
            html.push_str(&format!("<a href=\"{}\">{}</a>", url, text));
        }
        if !source_links.is_empty() && !fac_links.is_empty() {
            html.push_str(": ");
        }
        // Facsimile links with & separators
        for (i, (text, url)) in fac_links.iter().enumerate() {
            if i > 0 {
                html.push_str("<br>");
            }
            html.push_str(&format!("<a href=\"{}\">{}</a>", url, text));
            if i < fac_links.len() - 1 {
                html.push_str("&nbsp;&amp;");
            }
        }
        html.push_str("</h3>");
        return Some(html);
    }

    // Simple format: just facsimile links (Ms-xxx files)
    let links: Vec<(String, String)> = link_re
        .captures_iter(rest)
        .map(|cap| {
            let text = cap[1].replace("\\[", "[").replace("\\]", "]");
            let url = cap[2].to_string();
            (text, url)
        })
        .collect();

    if links.is_empty() {
        return None;
    }

    let mut html = String::from("<h3>");
    for (i, (text, url)) in links.iter().enumerate() {
        if i > 0 {
            html.push_str("<br>");
        }
        html.push_str(&format!("<a href=\"{}\">{}</a>", url, text));
        if i < links.len() - 1 {
            html.push_str("&nbsp;&amp;");
        }
    }
    html.push_str("</h3>");
    Some(html)
}

fn yaml_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
