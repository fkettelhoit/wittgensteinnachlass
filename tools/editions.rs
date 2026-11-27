//! The editor note of the published works (W-*), as on the site: "Edited by … (year).",
//! a revised edition or later title if there is one, and how the text differs from the
//! printed book. Shared by the ebooks and pdfs tools
//! (`#[path = "../../editions.rs"] mod editions;`). The editors are in
//! site/data/editions.yaml, the wording in site/data/edition_note.yaml next to it — the
//! same files the site's layouts/partials/edition-note.html reads.

use std::collections::HashMap;
use std::path::Path;

/// The editorial criteria on the site, for {about} in the wording.
const ABOUT: &str = "https://www.wittgensteinnachlass.com/about/#editorial-criteria";

/// site/data/edition_note.yaml: top-level `key: "value"` lines.
fn wording(path: &Path) -> Option<HashMap<String, String>> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut out = HashMap::new();
    for line in text.lines() {
        if line.starts_with('#') || line.starts_with(' ') {
            continue;
        }
        if let Some((key, value)) = line.split_once(": ") {
            let value = value.trim();
            let value = value.strip_prefix('"').and_then(|v| v.strip_suffix('"')).unwrap_or(value);
            out.insert(key.trim().to_string(), value.replace("\\\"", "\""));
        }
    }
    Some(out)
}

/// site/data/editions.yaml: work id -> field -> value. A deliberately small parser for
/// that file's format: top-level `W-XX:` keys, `  field: value` lines, and folded
/// (`>-`) multi-line values.
fn parse(yaml: &str) -> HashMap<String, HashMap<String, String>> {
    let mut works: HashMap<String, HashMap<String, String>> = HashMap::new();
    let (mut work, mut folded): (Option<String>, Option<String>) = (None, None);
    for line in yaml.lines() {
        let content = line.split(" #").next().unwrap_or(line);
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        if !line.starts_with(' ') {
            work = Some(content.trim().trim_end_matches(':').to_string());
            folded = None;
            continue;
        }
        let Some(w) = &work else { continue };
        let fields = works.entry(w.clone()).or_default();
        if let Some(f) = &folded {
            if line.starts_with("    ") {
                let v = fields.entry(f.clone()).or_default();
                if !v.is_empty() {
                    v.push(' ');
                }
                v.push_str(line.trim());
                continue;
            }
            folded = None;
        }
        if let Some((key, value)) = line.trim().split_once(':') {
            let value = value.trim();
            if value == ">-" {
                folded = Some(key.to_string());
                fields.insert(key.to_string(), String::new());
            } else {
                fields.insert(key.to_string(), value.to_string());
            }
        }
    }
    works
}

/// The note's Markdown for a work file (by stem, e.g. "W-RPP-2"), or None for documents.
/// Like the site, a work uses the longest key that is its stem or a prefix of it
/// followed by "-" (W-LW-2-MS-169 uses W-LW-2, then W-LW).
pub fn note(stem: &str, editions: &Path) -> Option<String> {
    if !stem.starts_with("W-") {
        return None;
    }
    let works = parse(&std::fs::read_to_string(editions).ok()?);
    let wording = wording(&editions.with_file_name("edition_note.yaml"))?;
    let key = works
        .keys()
        .filter(|k| stem == k.as_str() || stem.starts_with(&format!("{k}-")))
        .max_by_key(|k| k.len());
    let credit = key.and_then(|k| {
        let e = &works[k];
        e.get("credit").cloned().or_else(|| {
            let extra = e.get("extra").map(|x| format!(", {x}")).unwrap_or_default();
            Some(wording.get("credit")?
                .replace("{editors}", e.get("editors")?)
                .replace("{year}", e.get("year")?)
                .replace("{extra}", &extra))
        })
    });
    // A revised edition or a later title, on a line of its own after the editors.
    let revision = key.and_then(|k| works[k].get("revision"));
    let mut md = String::new();
    match (credit, revision) {
        (Some(c), Some(r)) => md.push_str(&format!("<p><em>{c}</em><br><em>{r}</em></p>\n\n")),
        (Some(c), None) => md.push_str(&format!("<p><em>{c}</em></p>\n\n")),
        _ => {}
    }
    md.push_str(&format!("<p>{}</p>\n\n", wording.get("note")?.replace("{about}", ABOUT)));
    Some(md)
}

/// Insert the note at the start of the body of pandoc input (after its YAML front matter).
pub fn with_note(content: &str, stem: &str, editions: &Path) -> String {
    let Some(note) = note(stem, editions) else {
        return content.to_string();
    };
    match content.strip_prefix("---\n").and_then(|rest| rest.find("\n---\n")) {
        Some(end) => {
            let split = 4 + end + "\n---\n".len();
            format!("{}\n{note}{}", &content[..split], content[split..].trim_start_matches('\n'))
        }
        None => format!("{note}{content}"),
    }
}
