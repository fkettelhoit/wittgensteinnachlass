use serde::Serialize;

/// One Meilisearch document = one remark, in one language.
///
/// `id` is the primary key: `{language}_{doc_slug}_{fragment}` (e.g. `de_ms-116_1321`).
/// `_` is the separator because Meilisearch ids allow only `[a-zA-Z0-9_-]` (no `:`), and no
/// component ever contains `_`. It is stable across rebuilds because it derives only from the
/// immutable page references, so re-runs update a remark in place rather than creating dupes.
#[derive(Serialize, Debug, Clone)]
pub struct SearchRecord {
    pub id: String,
    /// `de` or `en`.
    pub language: String,
    /// Document name, e.g. `Ms-116`.
    pub doc: String,
    /// Lowercased document name = URL path segment, e.g. `ms-116`.
    pub doc_slug: String,
    /// `Ms` or `Ts`.
    pub doctype: String,
    /// Position in canonical document + page order, for client-side result sorting.
    pub ord: u32,
    /// Facsimile page references as shown to readers, e.g. `["1[3]", "2[1]"]`.
    pub page_refs: Vec<String>,
    /// Goldmark heading anchor on the rendered page, e.g. `1321`.
    pub fragment: String,
    /// Deep link to the remark: `/ms-116/#1321` (de) or `/en/ms-116/#1321` (en).
    pub url: String,
    /// First facsimile image (full-resolution webp on the CDN).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    /// ISO date the remark was written, e.g. `1914-08-10`, when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    /// `YYYYMMDD` as an integer — sorts chronologically (Meili sorts numbers, not date strings).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_sort: Option<u64>,
    /// Published series number when the remark carries one, e.g. `1`, `1˙1`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series_number: Option<String>,
    /// Plain-text remark content — the primary searchable field.
    pub content: String,
    /// Works this remark was published in, each with a deep link to the remark on that work's
    /// page (empty if none).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub works: Vec<WorkLink>,
}

/// A work a remark appears in, with a deep link to the remark on the work's (part) page.
#[derive(Serialize, Debug, Clone)]
pub struct WorkLink {
    /// Work-level code, e.g. `W-RFM` — used for filtering (`works.code`), not display.
    pub code: String,
    /// Display label exactly as the document pages show it, e.g. `RFM III`, `PI`.
    pub label: String,
    /// Deep link to the remark within the work, e.g. `/w-rfm-3/#ms-122-5r25v1`.
    pub url: String,
}
