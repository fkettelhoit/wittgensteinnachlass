use regex::Regex;
use std::path::Path;

pub struct PreparedMarkdown {
    pub title: String,
    pub content: String,
}

pub struct Part {
    pub chapter_name: String,
    pub slug: String,
}

/// Check if a markdown file is a parent work (just links to parts).
/// Returns the title and list of parts if so.
pub fn detect_parent(raw: &str) -> Option<(String, Vec<Part>)> {
    let link_re = Regex::new(r"^- \[([^\]]+)\]\(/([^/]+)/\)$").unwrap();

    let mut title = String::new();
    let mut parts = Vec::new();
    let mut in_details = false;

    for line in raw.lines() {
        if title.is_empty() && line.starts_with("# ") {
            title = line[2..].trim().to_string();
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
        if in_details || line.trim().is_empty() {
            continue;
        }
        if let Some(caps) = link_re.captures(line) {
            let link_text = caps[1].to_string();
            let slug = caps[2].to_string();
            // Extract chapter name: everything after " – " in the link text,
            // or the full text if there's no separator
            let chapter_name = link_text
                .split(" – ")
                .nth(1)
                .unwrap_or(&link_text)
                .to_string();
            parts.push(Part { chapter_name, slug });
        } else {
            // Non-empty, non-link, non-details line — not a parent file
            return None;
        }
    }

    if parts.len() >= 2 {
        Some((title, parts))
    } else {
        None
    }
}

/// Convert a slug like "w-rfm-1-app-1" to a filename by finding a case-insensitive
/// match in the input directory.
pub fn slug_to_filename(slug: &str, input_dir: &Path) -> Option<String> {
    let target = slug.to_lowercase();
    if let Ok(entries) = std::fs::read_dir(input_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".md") {
                let stem = name.trim_end_matches(".md").to_lowercase();
                if stem == target {
                    return Some(name);
                }
            }
        }
    }
    None
}

/// Prepare a single part's body (no frontmatter, no title heading).
/// Returns the body lines as a string.
pub fn prepare_body(raw: &str) -> String {
    let mut body_lines: Vec<&str> = Vec::new();
    let mut found_title = false;
    let mut in_details = false;

    let h3_re = Regex::new(r"^### \[").unwrap();

    for line in raw.lines() {
        if !found_title && line.starts_with("# ") {
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

        if h3_re.is_match(line) {
            continue;
        }

        body_lines.push(line);
    }

    // Remove leading empty lines
    while body_lines.first().map_or(false, |l| l.trim().is_empty()) {
        body_lines.remove(0);
    }

    let mut content = String::new();
    for line in &body_lines {
        content.push_str(&line.replace("../graphics/", "../graphics-cropped/"));
        content.push('\n');
    }
    content
}

/// Prepare a merged multi-part work for pandoc EPUB conversion.
pub fn prepare_merged(title: &str, parts: &[(String, String)], author: &str) -> PreparedMarkdown {
    let mut content = String::new();
    content.push_str("---\n");
    content.push_str(&format!("title: \"{}\"\n", yaml_escape(title)));
    content.push_str(&format!("author: \"{}\"\n", yaml_escape(author)));
    content.push_str("lang: de\n");
    content.push_str("---\n\n");

    for (chapter_name, body) in parts {
        content.push_str(&format!("# {}\n\n", chapter_name));
        content.push_str(body);
        content.push('\n');
    }

    PreparedMarkdown {
        title: title.to_string(),
        content,
    }
}

/// Prepare a markdown file for pandoc EPUB conversion.
/// Adds YAML frontmatter, strips details blocks and page-ref headings.
pub fn prepare(raw: &str, author: &str) -> PreparedMarkdown {
    // Extract title from first "# " line
    let mut title = String::new();
    for line in raw.lines() {
        if line.starts_with("# ") {
            title = line[2..].trim().to_string();
            break;
        }
    }

    let body = prepare_body(raw);

    let mut content = String::new();
    content.push_str("---\n");
    content.push_str(&format!("title: \"{}\"\n", yaml_escape(&title)));
    content.push_str(&format!("author: \"{}\"\n", yaml_escape(author)));
    content.push_str("lang: de\n");
    content.push_str("---\n\n");
    content.push_str(&body);

    PreparedMarkdown { title, content }
}

fn yaml_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
