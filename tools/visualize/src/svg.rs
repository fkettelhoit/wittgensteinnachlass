use crate::parse::{Correspondence, RemarkBreak, SourceDoc, Work};
use std::collections::{HashMap, HashSet};

const REMARK_SPACING: f64 = 0.8;
const TRUNCATION_GAP: f64 = 6.0;
const LINE_LEN: f64 = 8.0;
const SEP_DASH_LEN: f64 = 8.0;
const TICK_EXTEND: f64 = 3.0;
const WORK_SOURCE_GAP: f64 = 380.0;
const DOC_GAP: f64 = 10.0;
const LABEL_INTERVAL: usize = 10;
const TITLE_OFFSET: f64 = 12.0;
const MARGIN_TOP: f64 = 10.0;
const MARGIN_BOTTOM: f64 = 10.0;
const MARGIN_H: f64 = 3.0;
const TITLE_FONT_SIZE: f64 = 11.0;
const LABEL_FONT_SIZE: f64 = 7.0;
/// Fixed label width for page identifiers (e.g. "100r", "213v").
const LABEL_WIDTH: f64 = 24.0;

/// Extract the page identifier from an anchor string.
/// E.g. "100r.2" → "100r", "80.2+81.1" → "80", "1.1" → "1"
fn page_from_anchor(anchor: &str) -> &str {
    let first_seg = anchor.split('+').next().unwrap_or(anchor);
    if let Some(dot_pos) = first_seg.rfind('.') {
        &first_seg[..dot_pos]
    } else {
        first_seg
    }
}

fn remarks_y(block_top: f64) -> f64 {
    block_top + TITLE_OFFSET
}

/// Which source remark indices have correspondences?
fn correspondence_indices(doc_name: &str, correspondences: &[Correspondence]) -> HashSet<usize> {
    correspondences
        .iter()
        .filter(|c| c.doc_name == doc_name)
        .map(|c| c.source_idx)
        .collect()
}

/// Determine which source remarks to show vs truncate.
/// Returns a vec of DisplayRemark for each original remark index.
enum DisplayEntry {
    Show(usize),      // show this remark (original index)
    Truncated(usize), // a truncation marker replacing N remarks
}

const MIN_CONTEXT: usize = 20;

fn compute_display_entries(count: usize, has_corr: &HashSet<usize>) -> Vec<DisplayEntry> {
    if count == 0 {
        return Vec::new();
    }

    // Mark indices that must be shown: correspondences + context around them
    let mut must_show = vec![false; count];
    for &idx in has_corr {
        let start = idx.saturating_sub(MIN_CONTEXT);
        let end = (idx + MIN_CONTEXT + 1).min(count);
        for slot in &mut must_show[start..end] {
            *slot = true;
        }
    }

    // Always show first and last MIN_CONTEXT remarks
    for slot in &mut must_show[..MIN_CONTEXT.min(count)] {
        *slot = true;
    }
    for slot in &mut must_show[count.saturating_sub(MIN_CONTEXT)..] {
        *slot = true;
    }

    // Build initial entries from must_show
    let mut entries: Vec<DisplayEntry> = Vec::new();
    let mut run_start: Option<usize> = None;
    let mut run_len = 0;

    for (i, &shown) in must_show.iter().enumerate() {
        if shown {
            if run_len > 0 {
                entries.push(DisplayEntry::Truncated(run_len));
                run_len = 0;
                run_start = None;
            }
            entries.push(DisplayEntry::Show(i));
        } else {
            if run_start.is_none() {
                run_start = Some(i);
            }
            run_len += 1;
        }
    }
    if run_len > 0 {
        entries.push(DisplayEntry::Truncated(run_len));
    }

    // Enforce invariant: every Truncated must have ≥ MIN_CONTEXT Show entries
    // before it and ≥ MIN_CONTEXT Show entries after it.
    // If not, expand by replacing Truncated with Show entries.
    loop {
        let mut changed = false;
        let mut new_entries = Vec::new();

        // Count Show entries before and after each Truncated
        let show_before: Vec<usize> = {
            let mut counts = Vec::with_capacity(entries.len());
            let mut running = 0;
            for entry in &entries {
                counts.push(running);
                if matches!(entry, DisplayEntry::Show(_)) {
                    running += 1;
                } else {
                    running = 0;
                }
            }
            counts
        };
        let show_after: Vec<usize> = {
            let mut counts = vec![0; entries.len()];
            let mut running = 0;
            for i in (0..entries.len()).rev() {
                counts[i] = running;
                if matches!(entries[i], DisplayEntry::Show(_)) {
                    running += 1;
                } else {
                    running = 0;
                }
            }
            counts
        };

        for (ei, entry) in entries.iter().enumerate() {
            match entry {
                DisplayEntry::Show(idx) => {
                    new_entries.push(DisplayEntry::Show(*idx));
                }
                DisplayEntry::Truncated(n) => {
                    let before = show_before[ei];
                    let after = show_after[ei];
                    let need_before = MIN_CONTEXT.saturating_sub(before);
                    let need_after = MIN_CONTEXT.saturating_sub(after);
                    let expand = need_before + need_after;

                    if expand >= *n {
                        // Not enough room to truncate — show all
                        // Find the starting index: look at the previous Show entry
                        let prev_idx = new_entries
                            .iter()
                            .rev()
                            .find_map(|e| {
                                if let DisplayEntry::Show(idx) = e {
                                    Some(*idx)
                                } else {
                                    None
                                }
                            })
                            .map(|i| i + 1)
                            .unwrap_or(0);
                        for j in 0..*n {
                            new_entries.push(DisplayEntry::Show(prev_idx + j));
                        }
                        changed = true;
                    } else if expand > 0 {
                        // Expand from both ends
                        let prev_idx = new_entries
                            .iter()
                            .rev()
                            .find_map(|e| {
                                if let DisplayEntry::Show(idx) = e {
                                    Some(*idx)
                                } else {
                                    None
                                }
                            })
                            .map(|i| i + 1)
                            .unwrap_or(0);
                        for j in 0..need_before {
                            new_entries.push(DisplayEntry::Show(prev_idx + j));
                        }
                        new_entries.push(DisplayEntry::Truncated(n - expand));
                        let after_start = prev_idx + n - need_after;
                        for j in 0..need_after {
                            new_entries.push(DisplayEntry::Show(after_start + j));
                        }
                        changed = true;
                    } else {
                        new_entries.push(DisplayEntry::Truncated(*n));
                    }
                }
            }
        }

        entries = new_entries;
        if !changed {
            break;
        }
    }

    entries
}

struct DocLayout {
    entry_ys: Vec<f64>,
    entries: Vec<DisplayEntry>,
    remark_y_map: HashMap<usize, f64>,
    separator_indices: HashSet<usize>,
    date_gap_indices: HashSet<usize>,
}

struct CorrespondenceRun {
    doc_name: String,
    work_start: usize,
    work_end: usize,
    source_start: usize,
    source_end: usize,
}

fn detect_runs(
    correspondences: &[Correspondence],
    source_layouts: &HashMap<String, DocLayout>,
) -> Vec<CorrespondenceRun> {
    let mut runs = Vec::new();
    let mut current: Option<CorrespondenceRun> = None;

    for corr in correspondences {
        let layout = match source_layouts.get(&corr.doc_name) {
            Some(l) => l,
            None => {
                if let Some(r) = current.take() {
                    runs.push(r);
                }
                continue;
            }
        };
        if !layout.remark_y_map.contains_key(&corr.source_idx) {
            if let Some(r) = current.take() {
                runs.push(r);
            }
            continue;
        }

        let has_break = layout.separator_indices.contains(&corr.source_idx)
            || layout.date_gap_indices.contains(&corr.source_idx);

        let extends = if let Some(ref c) = current {
            c.doc_name == corr.doc_name
                && corr.work_idx == c.work_end + 1
                && corr.source_idx == c.source_end + 1
                && !has_break
        } else {
            false
        };

        if extends {
            let c = current.as_mut().unwrap();
            c.work_end = corr.work_idx;
            c.source_end = corr.source_idx;
        } else {
            if let Some(r) = current.take() {
                runs.push(r);
            }
            current = Some(CorrespondenceRun {
                doc_name: corr.doc_name.clone(),
                work_start: corr.work_idx,
                work_end: corr.work_idx,
                source_start: corr.source_idx,
                source_end: corr.source_idx,
            });
        }
    }
    if let Some(r) = current {
        runs.push(r);
    }
    runs
}

fn layout_source_docs(
    doc_order: &[String],
    source_docs: &HashMap<String, SourceDoc>,
    correspondences: &[Correspondence],
) -> (HashMap<String, DocLayout>, f64) {
    let mut layouts = HashMap::new();
    let mut block_top = MARGIN_TOP;

    for doc_name in doc_order {
        let Some(doc) = source_docs.get(doc_name) else {
            continue;
        };
        let count = doc.anchors.len();
        let has_corr = correspondence_indices(doc_name, correspondences);

        // Build separator/gap sets from the breaks data
        let mut sep_set = HashSet::new();
        let mut gap_set = HashSet::new();
        for (i, brk) in doc.breaks.iter().enumerate() {
            match brk {
                RemarkBreak::Separator => {
                    sep_set.insert(i + 1);
                }
                RemarkBreak::DateGap => {
                    gap_set.insert(i + 1);
                }
                RemarkBreak::None => {}
            }
        }

        let display_entries = compute_display_entries(count, &has_corr);

        let mut entry_ys = Vec::new();
        let mut remark_y_map = HashMap::new();
        let mut y = remarks_y(block_top);
        let mut prev_orig_idx: Option<usize> = None;

        for entry in &display_entries {
            match entry {
                DisplayEntry::Show(orig_idx) => {
                    if prev_orig_idx.is_some() {
                        y += REMARK_SPACING;
                    }
                    entry_ys.push(y);
                    remark_y_map.insert(*orig_idx, y);
                    prev_orig_idx = Some(*orig_idx);
                }
                DisplayEntry::Truncated(_n) => {
                    y += TRUNCATION_GAP;
                    entry_ys.push(y);
                    y += TRUNCATION_GAP;
                    prev_orig_idx = None; // reset — next Show gets normal spacing
                }
            }
        }

        let block_height = if !entry_ys.is_empty() {
            y - block_top + REMARK_SPACING
        } else {
            TITLE_OFFSET
        };

        layouts.insert(
            doc_name.clone(),
            DocLayout {
                entry_ys,
                entries: display_entries,
                remark_y_map,
                separator_indices: sep_set,
                date_gap_indices: gap_set,
            },
        );

        block_top += block_height + DOC_GAP;
    }

    (layouts, block_top)
}

pub fn render(
    work: &Work,
    doc_order: &[String],
    source_docs: &HashMap<String, SourceDoc>,
    correspondences: &[Correspondence],
    font_base64: &str,
) -> String {
    let work_count = work.remarks.len();
    // Work column starts flush at the left edge
    let work_x = 0.0;
    let source_x = work_x + WORK_SOURCE_GAP;

    let work_first_y = remarks_y(MARGIN_TOP);
    let work_block_height = TITLE_OFFSET + work_count as f64 * REMARK_SPACING;

    let (source_layouts, source_total) =
        layout_source_docs(doc_order, source_docs, correspondences);

    let content_height = work_block_height.max(source_total - MARGIN_TOP);
    let svg_height = MARGIN_TOP + content_height + MARGIN_BOTTOM;
    let label_x = source_x + LINE_LEN + SEP_DASH_LEN + 3.0;
    let svg_width = label_x + LABEL_WIDTH + MARGIN_H;

    let mid_x = (work_x + LINE_LEN + source_x) / 2.0;

    let mut svg = String::new();

    svg.push_str(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{svg_width}" height="{svg_height}" viewBox="0 0 {svg_width} {svg_height}">
<style>
  @font-face {{
    font-family: "TeX Gyre Pagella";
    src: url("data:font/opentype;base64,{font_base64}") format("opentype");
  }}
  text {{ font-family: "TeX Gyre Pagella", "Palatino Linotype", "Book Antiqua", Palatino, serif; }}
  .title {{ font-size: {TITLE_FONT_SIZE}px; font-weight: bold; }}
  .label {{ font-size: {LABEL_FONT_SIZE}px; }}
  .fill {{ fill: #000; stroke: none; }}
  .curve {{ fill: #000; fill-opacity: 0.3; stroke: none; }}
</style>
<rect width="100%" height="100%" fill="white"/>
"#
    ));

    // Work title
    let title_y = MARGIN_TOP + TITLE_FONT_SIZE * 0.8;
    svg.push_str(&format!(
        r#"<text x="{work_x}" y="{title_y}" class="title">{}</text>
"#,
        xml_escape(&work.filename.replace(".md", ""))
    ));

    // Work remarks (left side) — single filled rectangle
    if work_count > 0 {
        let work_last_y = work_first_y + (work_count - 1) as f64 * REMARK_SPACING;
        let half = 0.25;
        svg.push_str(&format!(
            r#"<rect x="{work_x}" y="{}" width="{LINE_LEN}" height="{}" class="fill"/>
"#,
            work_first_y - half,
            work_last_y - work_first_y + 2.0 * half
        ));
    }

    // Source documents (right side)
    for doc_name in doc_order {
        let Some(layout) = source_layouts.get(doc_name) else {
            continue;
        };
        if layout.entry_ys.is_empty() {
            continue;
        }
        let first_y = layout.entry_ys[0];

        // Doc title
        let doc_title_y = first_y - TITLE_OFFSET + TITLE_FONT_SIZE * 0.8;
        svg.push_str(&format!(
            r#"<text x="{source_x}" y="{doc_title_y}" class="title">{}</text>
"#,
            xml_escape(doc_name)
        ));

        // Draw filled rectangles for contiguous runs of Show entries
        let half = 0.25;
        let mut range_start: Option<f64> = None;
        let mut range_end: f64 = 0.0;
        for (ei, entry) in layout.entries.iter().enumerate() {
            let y = layout.entry_ys[ei];
            match entry {
                DisplayEntry::Show(_) => {
                    if range_start.is_none() {
                        range_start = Some(y);
                    }
                    range_end = y;
                }
                DisplayEntry::Truncated(_) => {
                    if let Some(start) = range_start.take() {
                        svg.push_str(&format!(
                            r#"<rect x="{source_x}" y="{}" width="{LINE_LEN}" height="{}" class="fill"/>
"#,
                            start - half,
                            range_end - start + 2.0 * half
                        ));
                    }
                }
            }
        }
        if let Some(start) = range_start {
            svg.push_str(&format!(
                r#"<rect x="{source_x}" y="{}" width="{LINE_LEN}" height="{}" class="fill"/>
"#,
                start - half,
                range_end - start + 2.0 * half
            ));
        }

        // Overlay sparse elements: ticks, break lines, labels, truncation markers
        for (ei, entry) in layout.entries.iter().enumerate() {
            let y = layout.entry_ys[ei];
            match entry {
                DisplayEntry::Show(orig_idx) => {
                    let num = orig_idx + 1;
                    let is_tick = num % LABEL_INTERVAL == 0;
                    let has_break = layout.separator_indices.contains(orig_idx)
                        || layout.date_gap_indices.contains(orig_idx);

                    if is_tick {
                        svg.push_str(&format!(
                            r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="0.5"/>
"#,
                            source_x + LINE_LEN, y,
                            source_x + LINE_LEN + TICK_EXTEND, y,
                            "#000"
                        ));
                        let page = source_docs
                            .get(doc_name)
                            .map(|d| page_from_anchor(&d.anchors[*orig_idx]))
                            .unwrap_or("");
                        svg.push_str(&format!(
                            r#"<text x="{label_x}" y="{y}" class="label" dominant-baseline="middle">{}</text>
"#,
                            xml_escape(page)
                        ));
                    }

                    if has_break {
                        svg.push_str(&format!(
                            r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="0.5" stroke-dasharray="1,1"/>
"#,
                            source_x - SEP_DASH_LEN, y,
                            source_x + LINE_LEN + SEP_DASH_LEN, y,
                            "#000"
                        ));
                    }
                }
                DisplayEntry::Truncated(_) => {
                    svg.push_str(&format!(
                        r#"<text x="{}" y="{y}" class="label" text-anchor="middle" dominant-baseline="middle">⋮</text>
"#,
                        source_x + LINE_LEN / 2.0
                    ));
                }
            }
        }
    }

    // Bezier curves — filled shapes per run of consecutive correspondences
    let runs = detect_runs(correspondences, &source_layouts);
    let curve_gap = 2.0;
    let lx = work_x + LINE_LEN + curve_gap;
    let rx = source_x - curve_gap;
    let half = 0.25;
    for (ri, run) in runs.iter().enumerate() {
        let layout = source_layouts.get(&run.doc_name).unwrap();
        let raw_first_work_y = work_first_y + run.work_start as f64 * REMARK_SPACING;
        let raw_last_work_y = work_first_y + run.work_end as f64 * REMARK_SPACING;
        let first_source_y = *layout.remark_y_map.get(&run.source_start).unwrap();
        let last_source_y = *layout.remark_y_map.get(&run.source_end).unwrap();

        // Extend work-side edges to fill gaps between adjacent runs
        let first_work_y = if ri > 0 {
            let prev = &runs[ri - 1];
            let prev_last = work_first_y + prev.work_end as f64 * REMARK_SPACING;
            (prev_last + raw_first_work_y) / 2.0
        } else {
            raw_first_work_y - half
        };
        let last_work_y = if ri + 1 < runs.len() {
            let next = &runs[ri + 1];
            let next_first = work_first_y + next.work_start as f64 * REMARK_SPACING;
            (raw_last_work_y + next_first) / 2.0
        } else {
            raw_last_work_y + half
        };

        svg.push_str(&format!(
            r#"<path d="M {lx},{first_work_y} C {mid_x},{first_work_y} {mid_x},{first_source_y} {rx},{first_source_y} L {rx},{last_source_y} C {mid_x},{last_source_y} {mid_x},{last_work_y} {lx},{last_work_y} Z" class="curve"/>
"#
        ));
    }

    svg.push_str("</svg>\n");
    svg
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
