//! Find textually similar remarks across all manuscripts and typescripts.
//!
//! Each remark is normalized into word tokens and broken into overlapping word
//! k-shingles. Pairs of remarks are scored by IDF-weighted containment:
//! the IDF weight of their shared shingles divided by the total IDF weight of the
//! shorter remark's shingles. Shingles that occur in very many remarks (stock phrases)
//! are ignored entirely.

mod normalize;
mod parse;

use clap::Parser;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

#[derive(Parser)]
#[command(about = "Find textually similar remarks and write them to a TSV file")]
struct Args {
    /// Directory containing the Ms-*.md and Ts-*.md files
    #[arg(long, default_value = "../../md")]
    md: PathBuf,
    /// Output TSV file
    #[arg(long, default_value = "../../similar/similar.tsv")]
    out: PathBuf,
    /// Shingle size in words
    #[arg(long, default_value_t = 3)]
    k: usize,
    /// Ignore shingles that occur in more than this many remarks
    #[arg(long, default_value_t = 100)]
    max_df: u32,
    /// Minimum number of shared (non-ignored) shingles
    #[arg(long, default_value_t = 3)]
    min_shared: u32,
    /// Minimum IDF-weighted containment score
    #[arg(long, default_value_t = 0.4)]
    min_score: f64,
    /// Keep a pair only if it is among the top N matches of at least one of its remarks
    #[arg(long, default_value_t = 20)]
    top: usize,
    /// Print N example pairs per score band instead of writing the output file
    #[arg(long)]
    samples: Option<usize>,
    /// With --samples: only show pairs sharing at most this many shingles
    #[arg(long)]
    samples_max_shared: Option<u32>,
}

struct Doc {
    siglum: String,
    /// Document name, e.g. `Ms-115`.
    doc: String,
    /// Date range (`YYYYMMDD`) from Bergen's XML, if known.
    date: Option<(String, String)>,
    tokens: Vec<String>,
    /// Sorted, deduplicated shingle hashes.
    shingles: Vec<u64>,
}

struct Pair {
    a: u32,
    b: u32,
    score: f64,
    shared: u32,
}

fn shingles(tokens: &[String], k: usize) -> Vec<u64> {
    let mut out: Vec<u64> = tokens
        .windows(k)
        .map(|w| {
            let mut h = DefaultHasher::new();
            w.hash(&mut h);
            h.finish()
        })
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

fn main() {
    let args = Args::parse();

    let mut files: Vec<PathBuf> = fs::read_dir(&args.md)
        .expect("cannot read md directory")
        .map(|e| e.unwrap().path())
        .filter(|p| {
            let name = p.file_name().unwrap().to_string_lossy();
            (name.starts_with("Ms-") || name.starts_with("Ts-")) && name.ends_with(".md")
        })
        .collect();
    files.sort();
    let dates = parse::load_dates(&args.md.join("dates.tsv"));

    let docs: Vec<Doc> = files
        .iter()
        .flat_map(|f| parse::parse_doc(f))
        .map(|r| {
            let tokens = normalize::tokens(&r.body);
            let shingles = shingles(&tokens, args.k);
            let date = dates.get(&r.date_key).cloned().flatten();
            let doc = r.siglum.split(',').next().unwrap().to_string();
            Doc { siglum: r.siglum, doc, date, tokens, shingles }
        })
        .collect();
    let undated = docs.iter().filter(|d| d.date.is_none()).count();
    if undated > 0 {
        eprintln!("warning: {undated} remarks without a date");
    }
    let n = docs.len();
    eprintln!("{} files, {} remarks", files.len(), n);

    // Document frequency of each shingle.
    let mut df: HashMap<u64, u32> = HashMap::new();
    for d in &docs {
        for &s in &d.shingles {
            *df.entry(s).or_insert(0) += 1;
        }
    }
    let idf = |s: &u64| -> Option<f64> {
        let f = df[s];
        (f <= args.max_df).then(|| (n as f64 / f as f64).ln())
    };

    // Total weight per remark, and postings for shingles that can produce a match.
    let weight: Vec<f64> = docs
        .iter()
        .map(|d| d.shingles.iter().filter_map(idf).sum())
        .collect();
    let mut postings: HashMap<u64, Vec<u32>> = HashMap::new();
    for (i, d) in docs.iter().enumerate() {
        for s in &d.shingles {
            let f = df[s];
            if f >= 2 && f <= args.max_df {
                postings.entry(*s).or_default().push(i as u32);
            }
        }
    }

    // Accumulate shared weight for every pair (a < b) that shares a shingle.
    let mut pairs: Vec<Pair> = Vec::new();
    let mut acc: HashMap<u32, (f64, u32)> = HashMap::new();
    for (a, d) in docs.iter().enumerate() {
        acc.clear();
        for s in &d.shingles {
            let Some(list) = postings.get(s) else { continue };
            let w = idf(s).unwrap();
            for &b in list.iter().filter(|&&b| b as usize > a) {
                let e = acc.entry(b).or_insert((0.0, 0));
                e.0 += w;
                e.1 += 1;
            }
        }
        for (&b, &(w, shared)) in &acc {
            if shared < args.min_shared {
                continue;
            }
            let score = w / weight[a].min(weight[b as usize]);
            if score >= args.min_score {
                pairs.push(Pair { a: a as u32, b, score, shared });
            }
        }
    }
    eprintln!("{} pairs above thresholds", pairs.len());
    // Fixed order, so that ties at the top-N cutoff resolve the same way on every run.
    pairs.sort_by_key(|p| (p.a, p.b));

    // Keep only pairs in the top N of at least one side.
    let mut per_remark: Vec<Vec<(f64, usize)>> = vec![Vec::new(); n];
    for (i, p) in pairs.iter().enumerate() {
        per_remark[p.a as usize].push((p.score, i));
        per_remark[p.b as usize].push((p.score, i));
    }
    let mut keep = vec![false; pairs.len()];
    for list in &mut per_remark {
        list.sort_by(|x, y| y.0.total_cmp(&x.0).then(x.1.cmp(&y.1)));
        for &(_, i) in list.iter().take(args.top) {
            keep[i] = true;
        }
    }
    let pairs: Vec<Pair> = pairs
        .into_iter()
        .zip(keep)
        .filter_map(|(p, k)| k.then_some(p))
        .collect();
    eprintln!("{} pairs after top-{} filter", pairs.len(), args.top);

    if let Some(per_band) = args.samples {
        let shown: Vec<Pair> = pairs
            .into_iter()
            .filter(|p| args.samples_max_shared.map_or(true, |m| p.shared <= m))
            .collect();
        print_samples(&docs, &shown, per_band);
        return;
    }

    let mut out = String::from("a\tb\tscore\tshared\torder\n");
    for p in &pairs {
        let (a, b) = (&docs[p.a as usize], &docs[p.b as usize]);
        writeln!(
            out,
            "{}\t{}\t{:.2}\t{}\t{}",
            a.siglum, b.siglum, p.score, p.shared, chronology(a, b)
        )
        .unwrap();
    }
    if let Some(dir) = args.out.parent() {
        fs::create_dir_all(dir).unwrap();
    }
    fs::write(&args.out, out).expect("cannot write output");
    eprintln!("wrote {}", args.out.display());
}

/// Chronological relation of `a` to `b` (where `a` comes first in document order):
/// `<` if `a` is earlier, `>` if later, `~` if their dates can't be ordered.
///
/// Within a document, position decides. Across documents, the remarks' date ranges
/// decide: disjoint ranges are ordered, and so are overlapping ranges where one both
/// starts and ends no later than the other. Identical or nested ranges stay `~`.
fn chronology(a: &Doc, b: &Doc) -> char {
    if a.doc == b.doc {
        return '<';
    }
    let (Some((a1, a2)), Some((b1, b2))) = (&a.date, &b.date) else {
        return '~';
    };
    if a1 == b1 && a2 == b2 {
        '~'
    } else if a2 < b1 || (a1 <= b1 && a2 <= b2) {
        '<'
    } else if b2 < a1 || (b1 <= a1 && b2 <= a2) {
        '>'
    } else {
        '~'
    }
}

/// Print a histogram of scores and evenly spaced example pairs from each 0.1 band.
fn print_samples(docs: &[Doc], pairs: &[Pair], per_band: usize) {
    let excerpt = |d: &Doc| {
        let s = d.tokens.join(" ");
        match s.char_indices().nth(400) {
            Some((i, _)) => format!("{}…", &s[..i]),
            None => s,
        }
    };
    for band in (0..10).rev() {
        let lo = band as f64 / 10.0;
        let in_band: Vec<&Pair> = pairs
            .iter()
            .filter(|p| p.score >= lo && (p.score < lo + 0.1 || band == 9))
            .collect();
        if in_band.is_empty() {
            continue;
        }
        println!("\n==================== score {:.1}–{:.1}: {} pairs ====================", lo, lo + 0.1, in_band.len());
        let step = (in_band.len() / per_band).max(1);
        for p in in_band.iter().step_by(step).take(per_band) {
            let (a, b) = (&docs[p.a as usize], &docs[p.b as usize]);
            println!("\n--- {} ↔ {}  score {:.2}, {} shared", a.siglum, b.siglum, p.score, p.shared);
            println!("A ({} words): {}", a.tokens.len(), excerpt(a));
            println!("B ({} words): {}", b.tokens.len(), excerpt(b));
        }
    }
}
