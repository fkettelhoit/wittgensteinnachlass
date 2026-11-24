//! Reproduce the heading anchor IDs of remark headings, so deep links point at the right
//! remark. The site derives them from the facsimile labels in the heading's
//! `<span class="fac">` (see `site/layouts/_default/_markup/render-heading.html`):
//! lowercase, `[` and `.` become `-`, `]` is dropped, and the `,` between refs becomes
//! `--`, so `1r[2],2r[1]` -> `1r-2--2r-1` and `IIr[1]` -> `iir-1`. Verified against the
//! rendered `site/public/*/index.html` ids by `--verify-public`.

/// The heading id of a remark, from the inner markdown of its `fac` span. Every label
/// counts, linked (`[1r\[2\]](url)`) or plain text (a facsimile missing at build time).
pub fn remark_id(fac: &str) -> String {
    fac.split(',')
        .map(|seg| {
            // `[label](url)` -> `label`; plain-text labels pass through.
            let label = match (seg.strip_prefix('['), seg.find("](")) {
                (Some(_), Some(end)) => &seg[1..end],
                _ => seg,
            };
            unescape_label(label)
                .replace(']', "")
                .replace(['[', '.'], "-")
                .to_lowercase()
        })
        .collect::<Vec<_>>()
        .join("--")
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
        assert_eq!(remark_id(r"[IIr\[1\]](u/IIr.webp)"), "iir-1");
        assert_eq!(remark_id(r"[1r\[2\]](u/1r.webp),[2r\[1\]](u/2r.webp)"), "1r-2--2r-1");
        assert_eq!(remark_id(r"[1\[1\]](u/1.webp)"), "1-1");
        // Refs that Goldmark's auto-ids collapsed ("1531") stay distinct.
        assert_eq!(remark_id(r"[1\[5\]](u/1.webp),[3\[1\]](u/3.webp)"), "1-5--3-1");
        assert_eq!(remark_id(r"[153\[1\]](u/153.webp)"), "153-1");
    }

    #[test]
    fn unlinked_labels_and_split_remarks() {
        assert_eq!(remark_id(r"74av\[4\],[75r\[1\]](u/75r.webp)"), "74av-4--75r-1");
        assert_eq!(remark_id(r"[102a\[1\].2](u/102a.webp)"), "102a-1-2");
    }

    #[test]
    fn unescape_strips_backslashes() {
        assert_eq!(unescape_label("1r\\[2\\]"), "1r[2]");
        assert_eq!(unescape_label("IIr\\[1\\]"), "IIr[1]");
    }
}
