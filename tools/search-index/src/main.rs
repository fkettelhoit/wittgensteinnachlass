mod meili;
mod parse;
mod record;
mod slug;

use clap::Parser;
use parse::Res;
use record::SearchRecord;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::exit;

#[derive(Parser)]
#[command(
    name = "search-index-nachlass",
    about = "Extract remarks from md/ + md-en/ and push them to a Meilisearch index"
)]
struct Cli {
    /// Directory of German markdown (one file per document/work).
    #[arg(long, default_value = "../../md")]
    md_dir: PathBuf,

    /// Directory of English translation markdown.
    #[arg(long, default_value = "../../md-en")]
    en_dir: PathBuf,

    /// Meilisearch instance URL, e.g. https://ms-xxxx.meilisearch.io
    #[arg(long, env = "MEILI_HOST")]
    meili_host: Option<String>,

    /// Meilisearch admin (write) API key.
    #[arg(long, env = "MEILI_ADMIN_KEY")]
    meili_key: Option<String>,

    /// Index name prefix; the live index is `<prefix>` and the build index `<prefix>-build`.
    #[arg(long, env = "MEILI_INDEX_PREFIX", default_value = "remarks")]
    index_prefix: String,

    /// Extract and report only — no network calls. Prints counts and a couple of sample records.
    #[arg(long)]
    dry_run: bool,

    /// Cross-check every computed fragment against the rendered Hugo output in this directory
    /// (e.g. `../../site/public`). Fails if any deep-link anchor is missing from the real page.
    #[arg(long)]
    verify_public: Option<PathBuf>,
}

fn main() {
    dotenvy::dotenv().ok();
    let cli = Cli::parse();

    let res = Res::new();
    let records = match build_records(&cli, &res) {
        Ok(r) => r,
        Err(e) => fail(&e),
    };

    if records.is_empty() {
        fail("No records extracted — check --md-dir / --en-dir.");
    }

    if let Some(public) = &cli.verify_public {
        if let Err(e) = verify_public(&records, public) {
            fail(&e);
        }
    }

    if cli.dry_run {
        report(&records);
        return;
    }

    let host = cli.meili_host.as_deref().unwrap_or_else(|| {
        fail("--meili-host / MEILI_HOST is required (or use --dry-run).")
    });
    let key = cli.meili_key.as_deref().unwrap_or_else(|| {
        fail("--meili-key / MEILI_ADMIN_KEY is required (or use --dry-run).")
    });

    if let Err(e) = meili::reindex(host, key, &cli.index_prefix, &records) {
        fail(&e);
    }
}

/// Walk both markdown directories and extract every indexable remark (works are read from the
/// document headings as the remarks are parsed).
fn build_records(cli: &Cli, res: &Res) -> Result<Vec<SearchRecord>, String> {
    // A single counter across both passes; each pass walks files in document order and remarks
    // in page order, so `ord` increases with (document, page) within each language.
    let mut ord: u32 = 0;
    let mut records = Vec::new();
    records.extend(records_for_dir(&cli.md_dir, "de", "/{slug}/#{frag}", res, &mut ord)?);
    records.extend(records_for_dir(&cli.en_dir, "en", "/en/{slug}/#{frag}", res, &mut ord)?);

    let with_works = records.iter().filter(|r| !r.works.is_empty()).count();

    // Fail fast (also in --dry-run) if any primary key would be rejected by Meilisearch,
    // which only accepts ids composed of [a-zA-Z0-9_-].
    if let Some(r) = records
        .iter()
        .find(|r| !r.id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'))
    {
        return Err(format!(
            "invalid Meilisearch document id {:?} (allowed characters: a-z A-Z 0-9 - _)",
            r.id
        ));
    }
    eprintln!(
        "Extracted {} records ({} with a work association).",
        records.len(),
        with_works
    );
    Ok(records)
}

/// Extract records from every indexable file in a directory.
fn records_for_dir(
    dir: &Path,
    language: &str,
    url_template: &str,
    res: &Res,
    ord: &mut u32,
) -> Result<Vec<SearchRecord>, String> {
    let mut out = Vec::new();
    if !dir.exists() {
        eprintln!("warning: {} does not exist — skipping {language}.", dir.display());
        return Ok(out);
    }
    for path in indexable_files(dir)? {
        let content = fs::read_to_string(&path)
            .map_err(|e| format!("reading {}: {e}", path.display()))?;
        let Some((meta, remarks)) = parse::parse_file(&content, res) else {
            continue;
        };
        for r in remarks {
            if r.content.is_empty() {
                continue; // e.g. English section titles with no translated body
            }
            let url = url_template
                .replace("{slug}", &meta.doc_slug)
                .replace("{frag}", &r.fragment);
            let this_ord = *ord;
            *ord += 1;
            out.push(SearchRecord {
                // Meilisearch document ids allow only [a-zA-Z0-9_-]. doc_slug, fragment and
                // language never contain `_`, so it is an unambiguous separator.
                id: format!("{language}_{}_{}", meta.doc_slug, r.fragment),
                language: language.to_string(),
                doc: meta.doc.clone(),
                doc_slug: meta.doc_slug.clone(),
                doctype: meta.doctype.clone(),
                ord: this_ord,
                page_refs: r.page_refs,
                fragment: r.fragment,
                url,
                image_url: r.image_url,
                date: r.date,
                date_sort: r.date_sort,
                series_number: r.series_number,
                content: r.content,
                works: r.works,
            });
        }
    }
    Ok(out)
}

/// Indexable document files: `.md`, excluding Work files and the aggregate/navigation pages.
fn indexable_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let skip = [
        "index.md",
        "all.md",
        "docs-by-date.md",
        "docs-by-name.md",
        "deepl-remarks.md",
        "about.md",
    ];
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|e| format!("reading dir {}: {e}", dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            name.ends_with(".md") && !name.starts_with("W-") && !skip.contains(&name)
        })
        .collect();
    // Document order, not lexicographic: Ms before Ts, numeric within (Ms-9 before Ms-10),
    // letter suffixes last (Ts-227a before Ts-227b). This is what `ord` records.
    files.sort_by(|a, b| doc_order_key(a).cmp(&doc_order_key(b)));
    Ok(files)
}

/// Sort key for a document file: (prefix, number, suffix), e.g. `Ms-227a` -> ("Ms", 227, "a").
/// Sorting tuples gives Ms before Ts, numeric order within a prefix, and suffixed docs last.
fn doc_order_key(path: &Path) -> (String, u64, String) {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let (prefix, rest) = stem.split_once('-').unwrap_or((stem, ""));
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    let num = digits.parse().unwrap_or(0);
    (prefix.to_string(), num, rest[digits.len()..].to_string())
}

/// Cross-check every record's deep-link anchor against the rendered Hugo page it points at.
/// This is the guard for the project's highest-risk assumption — that our fragment slugs match
/// Goldmark's heading IDs exactly. Pages absent from `public` (e.g. a partial local build) are
/// skipped, not failed.
fn verify_public(records: &[SearchRecord], public: &Path) -> Result<(), String> {
    use std::collections::{BTreeMap, HashMap};
    let mut cache: HashMap<PathBuf, Option<String>> = HashMap::new();
    let (mut checked, mut skipped, mut missing_total) = (0usize, 0usize, 0usize);
    // doc_slug -> (missing anchors, total anchors checked) so we can see whether a failing
    // page is simply truncated/incomplete (missing ~ total) vs a real slug mismatch.
    let mut per_doc: BTreeMap<String, (usize, usize)> = BTreeMap::new();

    for r in records {
        let rel = if r.language == "en" {
            format!("en/{}/index.html", r.doc_slug)
        } else {
            format!("{}/index.html", r.doc_slug)
        };
        let path = public.join(rel);
        let html = cache
            .entry(path.clone())
            .or_insert_with(|| fs::read_to_string(&path).ok());
        let Some(html) = html else {
            skipped += 1;
            continue;
        };
        checked += 1;
        let key = format!("{}{}", if r.language == "en" { "en/" } else { "" }, r.doc_slug);
        let entry = per_doc.entry(key).or_insert((0, 0));
        entry.1 += 1;
        if !html.contains(&format!("id=\"{}\"", r.fragment)) {
            missing_total += 1;
            entry.0 += 1;
        }
    }

    eprintln!("verify: checked {checked} anchors against {}, skipped {skipped} (no rendered page).", public.display());
    if missing_total > 0 {
        eprintln!("verify: {missing_total} anchor(s) not found, in these pages (missing/total — `=` means the rendered page is truncated):");
        for (doc, (miss, total)) in &per_doc {
            if *miss > 0 {
                let flag = if miss == total { "  [page fully missing → stale/truncated render]" } else { "" };
                eprintln!("  {doc}: {miss}/{total}{flag}");
            }
        }
        return Err("computed fragments missing from rendered pages (see breakdown above)".to_string());
    }
    eprintln!("verify: all {checked} anchors present in the rendered pages ✓");
    Ok(())
}

/// Print a dry-run summary plus a couple of sample records.
fn report(records: &[SearchRecord]) {
    let de = records.iter().filter(|r| r.language == "de").count();
    let en = records.iter().filter(|r| r.language == "en").count();
    let with_date = records.iter().filter(|r| r.date.is_some()).count();
    let with_works = records.iter().filter(|r| !r.works.is_empty()).count();
    println!("Total records: {}", records.len());
    println!("  de: {de}   en: {en}");
    println!("  with date: {with_date}   with work: {with_works}");
    println!("\nSample records:");
    let mut shown_en = false;
    let mut shown_de = false;
    for r in records {
        if (r.language == "de" && !shown_de) || (r.language == "en" && !shown_en) {
            println!("{}", serde_json::to_string_pretty(r).unwrap());
            if r.language == "de" {
                shown_de = true;
            } else {
                shown_en = true;
            }
        }
        if shown_de && shown_en {
            break;
        }
    }
}

fn fail(msg: &str) -> ! {
    eprintln!("error: {msg}");
    exit(1);
}
