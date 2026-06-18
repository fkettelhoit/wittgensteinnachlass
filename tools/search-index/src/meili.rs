//! Minimal Meilisearch REST client and the atomic full-reindex flow.
//!
//! The site is rebuilt from scratch on every deploy, so the index must be a complete,
//! orphan-free rebuild. We build into a throwaway `<prefix>-build` index and then atomically
//! `swap-indexes` it with the live `<prefix>` index, so searchers always hit a complete index
//! and a half-finished build can never corrupt the live one.

use crate::record::SearchRecord;
use reqwest::blocking::Client;
use reqwest::StatusCode;
use serde_json::{json, Value};
use std::thread::sleep;
use std::time::{Duration, Instant};

const BATCH_SIZE: usize = 5_000;
const TASK_TIMEOUT: Duration = Duration::from_secs(600);

struct Meili {
    client: Client,
    host: String,
    key: String,
}

impl Meili {
    fn new(host: &str, key: &str) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .expect("building HTTP client");
        Meili {
            client,
            host: host.trim_end_matches('/').to_string(),
            key: key.to_string(),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.host, path)
    }

    /// Send a request, treat any non-2xx as an error (surfacing the response body), and parse
    /// the JSON body. `allow_404` lets callers distinguish "missing" from a real failure.
    fn send(&self, builder: reqwest::blocking::RequestBuilder, allow_404: bool) -> Result<(StatusCode, Value), String> {
        let resp = builder
            .bearer_auth(&self.key)
            .send()
            .map_err(|e| format!("request failed: {e}"))?;
        let status = resp.status();
        if status == StatusCode::NOT_FOUND && allow_404 {
            return Ok((status, Value::Null));
        }
        let body = resp.text().map_err(|e| format!("reading response body: {e}"))?;
        if !status.is_success() {
            return Err(format!("Meilisearch returned {status}: {body}"));
        }
        let value = if body.is_empty() {
            Value::Null
        } else {
            serde_json::from_str(&body).map_err(|e| format!("parsing response: {e} (body: {body})"))?
        };
        Ok((status, value))
    }

    fn index_exists(&self, uid: &str) -> Result<bool, String> {
        let (status, _) = self.send(self.client.get(self.url(&format!("/indexes/{uid}"))), true)?;
        Ok(status != StatusCode::NOT_FOUND)
    }

    fn create_index(&self, uid: &str) -> Result<(), String> {
        let (_, v) = self.send(
            self.client
                .post(self.url("/indexes"))
                .json(&json!({ "uid": uid, "primaryKey": "id" })),
            false,
        )?;
        self.wait_task(task_uid(&v)?)
    }

    fn delete_index(&self, uid: &str) -> Result<(), String> {
        let (_, v) = self.send(self.client.delete(self.url(&format!("/indexes/{uid}"))), false)?;
        self.wait_task(task_uid(&v)?)
    }

    /// Ensure an index exists (create it empty if not). Needed for the live index so the
    /// first-ever run has something to swap against.
    fn ensure_index(&self, uid: &str) -> Result<(), String> {
        if !self.index_exists(uid)? {
            eprintln!("Creating index {uid}");
            self.create_index(uid)?;
        }
        Ok(())
    }

    /// Drop and recreate an index so it starts from a clean, empty state.
    fn recreate_index(&self, uid: &str) -> Result<(), String> {
        if self.index_exists(uid)? {
            self.delete_index(uid)?;
        }
        self.create_index(uid)
    }

    fn update_settings(&self, uid: &str, settings: &Value) -> Result<(), String> {
        let (_, v) = self.send(
            self.client
                .patch(self.url(&format!("/indexes/{uid}/settings")))
                .json(settings),
            false,
        )?;
        self.wait_task(task_uid(&v)?)
    }

    fn add_documents(&self, uid: &str, batch: &[&SearchRecord]) -> Result<u64, String> {
        let (_, v) = self.send(
            self.client
                .post(self.url(&format!("/indexes/{uid}/documents")))
                .json(batch),
            false,
        )?;
        task_uid(&v)
    }

    fn swap(&self, a: &str, b: &str) -> Result<(), String> {
        let (_, v) = self.send(
            self.client
                .post(self.url("/swap-indexes"))
                .json(&json!([{ "indexes": [a, b] }])),
            false,
        )?;
        self.wait_task(task_uid(&v)?)
    }

    /// Poll a task until it succeeds; error (with Meilisearch's own message) if it fails.
    fn wait_task(&self, uid: u64) -> Result<(), String> {
        let start = Instant::now();
        loop {
            let (_, v) = self.send(self.client.get(self.url(&format!("/tasks/{uid}"))), false)?;
            match v.get("status").and_then(Value::as_str) {
                Some("succeeded") => return Ok(()),
                Some("failed") | Some("canceled") => {
                    return Err(format!("task {uid} failed: {}", v.get("error").unwrap_or(&v)));
                }
                _ => {}
            }
            if start.elapsed() > TASK_TIMEOUT {
                return Err(format!("task {uid} did not finish within {TASK_TIMEOUT:?}"));
            }
            sleep(Duration::from_millis(400));
        }
    }
}

fn task_uid(v: &Value) -> Result<u64, String> {
    v.get("taskUid")
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("no taskUid in response: {v}"))
}

/// Index settings applied to the build index (carried to the live index by the swap).
fn settings() -> Value {
    json!({
        "searchableAttributes": ["content", "page_refs", "doc"],
        "filterableAttributes": ["language", "doctype", "doc", "doc_slug", "works", "date_sort"],
        "sortableAttributes": ["date_sort"],
        // German compounds are long, so keep the two-typo threshold high to avoid noise.
        "typoTolerance": { "minWordSizeForTypos": { "oneTypo": 5, "twoTypos": 9 } },
        // Conservative stop-word list: function words can be load-bearing in a philosophy
        // corpus ("das Wort 'ist'"), so under-stop rather than over-stop. Tunable later.
        "stopWords": [
            "der", "die", "das", "den", "dem", "des", "ein", "eine", "einer", "und", "oder",
            "the", "a", "an", "and", "or", "of", "to"
        ]
    })
}

/// Full reindex: ensure the live index exists, rebuild into `<prefix>-build`, then swap.
pub fn reindex(host: &str, key: &str, prefix: &str, records: &[SearchRecord]) -> Result<(), String> {
    let meili = Meili::new(host, key);
    let live = prefix.to_string();
    let build = format!("{prefix}-build");

    meili.ensure_index(&live)?;
    eprintln!("Rebuilding into {build}");
    meili.recreate_index(&build)?;
    meili.update_settings(&build, &settings())?;

    let refs: Vec<&SearchRecord> = records.iter().collect();
    let mut tasks = Vec::new();
    for (i, chunk) in refs.chunks(BATCH_SIZE).enumerate() {
        let uid = meili.add_documents(&build, chunk)?;
        tasks.push(uid);
        eprintln!("  queued batch {} ({} docs, task {uid})", i + 1, chunk.len());
    }
    for uid in tasks {
        meili.wait_task(uid)?;
    }

    eprintln!("Swapping {build} into {live}");
    meili.swap(&live, &build)?;
    eprintln!("Done: {} documents live in {live}.", records.len());
    Ok(())
}
