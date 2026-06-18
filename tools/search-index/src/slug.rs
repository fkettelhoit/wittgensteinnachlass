//! Reproduce the heading anchor IDs Hugo/Goldmark generates, so deep links point at the
//! right remark. Hugo's default `autoHeadingIDType` is GitHub-style: lowercase, drop every
//! character that is not a letter, number, space or hyphen, then turn runs of spaces/hyphens
//! into a single hyphen and trim leading/trailing hyphens.
//!
//! For our facsimile headings (e.g. `1r[2],2r[1]` or `IIr[1]`) only ASCII alphanumerics
//! survive, so `1r[2]2r[1]` -> `1r22r1` and `IIr[1]` -> `iir1`. Verified against rendered
//! `site/public/*/index.html` ids — see the tests.

/// GitHub-style anchor slug, matching Hugo's default heading-ID generation.
pub fn github_slug(text: &str) -> String {
    let mut out = String::new();
    let mut pending_hyphen = false;
    for c in text.chars() {
        if c.is_alphanumeric() {
            if pending_hyphen && !out.is_empty() {
                out.push('-');
            }
            pending_hyphen = false;
            out.extend(c.to_lowercase());
        } else if c == ' ' || c == '-' || c == '_' {
            // Underscores are kept by GitHub but collapse with spaces/hyphens into one '-'.
            if c == '_' {
                if pending_hyphen && !out.is_empty() {
                    out.push('-');
                }
                pending_hyphen = false;
                out.push('_');
            } else {
                pending_hyphen = true;
            }
        }
        // All other characters (brackets, commas, dots, plus signs, …) are dropped.
    }
    out
}

/// Undo the markdown bracket-escaping used in facsimile labels (`1r\[2\]` -> `1r[2]`).
pub fn unescape_label(label: &str) -> String {
    label.replace("\\[", "[").replace("\\]", "]")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_rendered_document_ids() {
        // From site/public/ms-101/ and ms-116/: the concatenated facsimile label text.
        assert_eq!(github_slug("IIr[1]"), "iir1");
        assert_eq!(github_slug("1r[2]2r[1]"), "1r22r1");
        assert_eq!(github_slug("1[1]"), "11");
        assert_eq!(github_slug("1[3]2[1]"), "1321");
        assert_eq!(github_slug("2r[3]3r[1]"), "2r33r1");
    }

    #[test]
    fn unescape_strips_backslashes() {
        assert_eq!(unescape_label("1r\\[2\\]"), "1r[2]");
        assert_eq!(unescape_label("IIr\\[1\\]"), "IIr[1]");
    }

    #[test]
    fn full_pipeline_matches() {
        // Build a fragment the way the parser does: unescape each label, concatenate, slug.
        let labels = ["1r\\[2\\]", "2r\\[1\\]"];
        let joined: String = labels.iter().map(|l| unescape_label(l)).collect();
        assert_eq!(github_slug(&joined), "1r22r1");
    }
}
