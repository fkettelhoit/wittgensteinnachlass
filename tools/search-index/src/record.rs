use serde::Serialize;

/// One Meilisearch document = one remark, in one language.
///
/// `id` is the primary key: `{language}:{doc_slug}:{fragment}` (e.g. `de:ms-116:1321`).
/// It is stable across rebuilds because it derives only from the immutable page references,
/// so re-runs update a remark in place rather than creating duplicates.
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
    /// Work codes this remark was published in, e.g. `["W-PI"]` (empty if none).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub works: Vec<String>,
}
