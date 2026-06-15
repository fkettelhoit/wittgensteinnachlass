use regex::Regex;
use serde_json::json;
use std::collections::HashMap;
use std::path::Path;
use std::{fs, io::Write};

/// Parsed glossary split into term entries and general sections.
pub struct Glossary {
    /// Lines of the form `German = English` paired with a lowercase German key for matching.
    terms: Vec<(String, String)>, // (lowercase_german, original_line)
    /// General principles and notes sections (always included).
    general: String,
}

impl Glossary {
    /// Parse a glossary markdown string into terms and general sections.
    pub fn parse(content: &str) -> Self {
        let mut terms = Vec::new();
        let mut general = String::new();
        let mut in_general = false;

        for line in content.lines() {
            if line.starts_with("## ") {
                in_general = true;
                general.push_str(line);
                general.push('\n');
            } else if in_general {
                general.push_str(line);
                general.push('\n');
            } else if line.contains(" = ") && !line.starts_with('#') {
                let german = line.split(" = ").next().unwrap().trim();
                // Strip parenthetical disambiguators from German side, e.g. "Zug (Spielzug)"
                let clean_german = if let Some(pos) = german.find(" (") {
                    &german[..pos]
                } else {
                    german
                };
                terms.push((clean_german.to_lowercase(), line.to_string()));
            }
        }

        Glossary { terms, general }
    }

    /// Return an empty glossary (no terms, no general sections).
    pub fn empty() -> Self {
        Glossary {
            terms: Vec::new(),
            general: String::new(),
        }
    }

    /// Whether the glossary has any term entries.
    pub fn has_terms(&self) -> bool {
        !self.terms.is_empty()
    }

    /// Normalize text for fuzzy matching: lowercase, strip periods and extra whitespace.
    fn normalize(s: &str) -> String {
        s.to_lowercase().replace('.', "").split_whitespace().collect::<Vec<_>>().join(" ")
    }

    /// Return the general translation principles (for inclusion in the system prompt).
    pub fn general_section(&self) -> &str {
        &self.general
    }

    /// Filter the glossary to only include terms that appear in the given text.
    /// Returns only matching term lines, or empty string if none match.
    pub fn filter_for(&self, text: &str) -> String {
        if self.terms.is_empty() {
            return String::new();
        }
        let text_norm = Self::normalize(text);
        let mut result = String::new();
        for (german_lower, line) in &self.terms {
            let term_norm = Self::normalize(german_lower);
            if text_norm.contains(&term_norm) {
                result.push_str(line);
                result.push('\n');
            }
        }
        result
    }
}

/// A remark: its heading line(s) and body text (trimmed).
pub struct Remark {
    pub heading: String,
    pub body: String,
}

/// Split a markdown document into its preamble and a list of remarks.
pub fn parse_document(content: &str) -> (String, Vec<Remark>) {
    let mut preamble = String::new();
    let mut remarks = Vec::new();
    let mut current_heading = String::new();
    let mut current_body = String::new();
    let mut in_remark = false;

    for line in content.lines() {
        if line.starts_with("### ") {
            if in_remark {
                remarks.push(Remark {
                    heading: current_heading.clone(),
                    body: current_body.trim().to_string(),
                });
            }
            current_heading = line.to_string();
            current_body = String::new();
            in_remark = true;
        } else if !in_remark {
            if !preamble.is_empty() {
                preamble.push('\n');
            }
            preamble.push_str(line);
        } else {
            if !current_body.is_empty() {
                current_body.push('\n');
            }
            current_body.push_str(line);
        }
    }
    if in_remark {
        remarks.push(Remark {
            heading: current_heading,
            body: current_body.trim().to_string(),
        });
    }

    (preamble, remarks)
}

/// Extract <math>...</math> blocks and replace with placeholders.
pub fn extract_math(text: &str) -> (String, Vec<String>) {
    let re = Regex::new(r"(?s)<math[\s>].*?</math>").unwrap();
    let mut blocks = Vec::new();
    let result = re.replace_all(text, |caps: &regex::Captures| {
        let placeholder = format!("\u{27E6}MATH:{}\u{27E7}", blocks.len() + 1);
        blocks.push(caps[0].to_string());
        placeholder
    });
    (result.into_owned(), blocks)
}

/// Restore math placeholders with original blocks.
pub fn restore_math(text: &str, blocks: &[String]) -> String {
    let mut result = text.to_string();
    for (i, block) in blocks.iter().enumerate() {
        let placeholder = format!("\u{27E6}MATH:{}\u{27E7}", i + 1);
        result = result.replacen(&placeholder, block, 1);
    }
    result
}

/// Convert _emphasis_ to <em>emphasis</em> and **bold** to <strong>bold</strong>.
/// The model preserves HTML tags more reliably than markdown emphasis markers.
pub fn emphasis_to_html(text: &str) -> String {
    let bold_re = Regex::new(r"\*\*([^*]+)\*\*").unwrap();
    let result = bold_re.replace_all(text, "<strong>$1</strong>");
    let em_re = Regex::new(r"_([^_]+)_").unwrap();
    em_re.replace_all(&result, "<em>$1</em>").into_owned()
}

/// Convert <em>emphasis</em> back to _emphasis_ and <strong>bold</strong> back to **bold**.
pub fn emphasis_from_html(text: &str) -> String {
    text.replace("<em>", "_")
        .replace("</em>", "_")
        .replace("<strong>", "**")
        .replace("</strong>", "**")
}

/// Split text into segments at sentence boundaries (. ? ! –) followed by uppercase/quote.
/// Emphasis never crosses these boundaries in the Wittgenstein corpus.
pub fn split_segments(text: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut start = 0;
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();

    for i in 0..len {
        // Look for sentence-ending punctuation followed by whitespace + uppercase/quote/paren
        if matches!(chars[i], '.' | '?' | '!' | '\u{2013}') {
            // Skip whitespace after punctuation
            let mut j = i + 1;
            while j < len && chars[j].is_whitespace() {
                j += 1;
            }
            // Check if next non-whitespace char starts a new sentence
            if j < len
                && j > i + 1
                && (chars[j].is_uppercase()
                    || matches!(chars[j], '\u{201e}' | '\u{201c}' | '('))
            {
                let byte_end: usize = chars[..j].iter().map(|c| c.len_utf8()).sum();
                let byte_start_old: usize = chars[..start].iter().map(|c| c.len_utf8()).sum();
                segments.push(text[byte_start_old..byte_end].trim().to_string());
                start = j;
            }
        }
    }
    // Push the last segment
    let byte_start: usize = chars[..start].iter().map(|c| c.len_utf8()).sum();
    let remaining = text[byte_start..].trim();
    if !remaining.is_empty() {
        segments.push(remaining.to_string());
    }
    if segments.is_empty() {
        segments.push(text.to_string());
    }
    segments
}

/// Count segments in the German text that contain emphasis markers (_ or **).
pub fn count_emphasized_segments(german: &str) -> usize {
    split_segments(german)
        .iter()
        .filter(|s| s.contains('_') || s.contains("**"))
        .count()
}

/// Attempt segment-by-segment emphasis repair using the LLM.
/// Returns the repaired English body, or None if repair wasn't possible.
pub fn repair_emphasis_by_segment(
    client: &reqwest::blocking::Client,
    ollama_url: &str,
    model: &str,
    num_ctx: usize,
    german_body: &str,
    english_body: &str,
    verbose: bool,
) -> Option<String> {
    let de_segs = split_segments(german_body);
    let en_segs = split_segments(english_body);

    if de_segs.len() != en_segs.len() {
        return None; // segment count mismatch, can't align
    }

    let system_msg = "Insert emphasis markers into the English text to match the German original. \
        Use _underscores_ where the German uses _underscores_, and **double asterisks** where \
        the German uses **double asterisks** (for intra-word emphasis). Do not change any words — \
        only add or remove emphasis markers. Output only the modified English text.";

    let mut fixed_segs = en_segs.clone();
    let mut changed = false;

    for (i, (de_seg, en_seg)) in de_segs.iter().zip(en_segs.iter()).enumerate() {
        let de_us = de_seg.chars().filter(|&c| c == '_').count();
        let en_us = en_seg.chars().filter(|&c| c == '_').count();
        let de_ast = de_seg.chars().filter(|&c| c == '*').count();
        let en_ast = en_seg.chars().filter(|&c| c == '*').count();

        if de_us == en_us && de_ast == en_ast {
            continue; // this segment is fine
        }
        if de_us == 0 && de_ast == 0 && (en_us > 0 || en_ast > 0) {
            // EN has spurious emphasis — strip it
            let stripped = en_seg.replace('_', "");
            // Also strip ** if DE has none
            let stripped = Regex::new(r"\*\*([^*]+)\*\*")
                .unwrap()
                .replace_all(&stripped, "$1")
                .to_string();
            fixed_segs[i] = stripped;
            changed = true;
            continue;
        }
        if de_us == 0 && de_ast == 0 {
            continue; // neither has emphasis, fine
        }

        // DE has emphasis, EN doesn't match — ask the LLM to fix
        // Strip existing EN emphasis to give the model a clean slate
        let en_clean = en_seg.replace('_', "");
        let en_clean = Regex::new(r"\*\*([^*]+)\*\*")
            .unwrap()
            .replace_all(&en_clean, "$1")
            .to_string();
        let user_msg = format!(
            "German: {}\nEnglish: {}",
            de_seg, en_clean
        );

        if verbose {
            eprint!(" [seg {}/{}]", i + 1, de_segs.len());
        }

        match call_ollama(client, ollama_url, model, system_msg, &user_msg, num_ctx) {
            Ok(result) => {
                let trimmed = result.trim().to_string();
                // Verify the fix has the right emphasis counts
                let result_us = trimmed.chars().filter(|&c| c == '_').count();
                let result_ast = trimmed.chars().filter(|&c| c == '*').count();
                if result_us == de_us && result_ast == de_ast {
                    fixed_segs[i] = trimmed;
                    changed = true;
                }
                // If the model got it wrong, keep the original EN segment
            }
            Err(_) => {} // keep original on error
        }
    }

    if changed {
        Some(fixed_segs.join(" "))
    } else {
        None
    }
}

/// Fix emphasis markers in translated text based on what the German original uses.
/// - Converts single *word* to _word_ (always wrong in our output)
/// - Converts **word** to _word_ if the German has no ** (model used bold instead of emphasis)
pub fn fix_emphasis_markers(translated: &str, german: &str) -> String {
    let mut text = translated.to_string();

    // If German has no **, convert all **word** to _word_ in English
    if !german.contains("**") {
        let bold_re = Regex::new(r"\*\*(.+?)\*\*").unwrap();
        text = bold_re.replace_all(&text, "_${1}_").into_owned();
    }

    // Convert remaining single *word* to _word_
    // Protect any legitimate ** first
    let protected = text.replace("**", "\u{FFFE}BOLD\u{FFFE}");
    let single_re = Regex::new(r"\*([^*]+)\*").unwrap();
    let fixed = single_re.replace_all(&protected, "_${1}_");
    let result = fixed.replace("\u{FFFE}BOLD\u{FFFE}", "**");

    // Fix HTML-encoded ampersands — the model sometimes outputs &amp; instead of &
    result.replace("&amp;", "&")
}

/// Extract all _emphasized_ and **bold** passages from text.
/// Returns a prompt fragment listing them, or empty string if none found.
pub fn emphasis_checklist(text: &str) -> String {
    let em_re = Regex::new(r"_([^_]+)_").unwrap();
    let bold_re = Regex::new(r"\*\*([^*]+)\*\*").unwrap();
    let mut passages: Vec<String> = Vec::new();
    for cap in em_re.captures_iter(text) {
        passages.push(format!("_{}_", &cap[1]));
    }
    for cap in bold_re.captures_iter(text) {
        passages.push(format!("**{}**", &cap[1]));
    }
    if passages.is_empty() {
        return String::new();
    }
    format!(
        "The input contains {} emphasized passage(s) wrapped in _underscores_ or **double asterisks**: {}.\n\
         Your translation MUST wrap the English translations of these exact words \
         in the same markers (_underscores_ or **double asterisks**).\n\n",
        passages.len(),
        passages.join(", ")
    )
}

/// Return the correct DeepL API base URL based on the API key.
/// Free keys end with ":fx" and use api-free.deepl.com.
pub fn deepl_api_base(api_key: &str) -> &'static str {
    if api_key.ends_with(":fx") {
        "https://api-free.deepl.com"
    } else {
        "https://api.deepl.com"
    }
}

/// Create a DeepL glossary from the term entries. Returns the glossary ID.
/// Check if a glossary entry has multiple translation options.
fn is_ambiguous(en_side: &str) -> bool {
    en_side.contains(" / ") || en_side.contains('/') || {
        // Check for multiple comma-separated translations (but not commas inside parentheticals)
        let no_parens = Regex::new(r"\([^)]*\)")
            .unwrap()
            .replace_all(en_side, "");
        no_parens.matches(", ").count() > 0
    }
}

/// Build context text from ambiguous glossary entries for use with DeepL's context parameter.
pub fn deepl_ambiguous_context(glossary: &Glossary) -> String {
    let mut lines = Vec::new();
    for (_german, line) in &glossary.terms {
        if let Some(pos) = line.find(" = ") {
            let en_side = &line[pos + 3..];
            if is_ambiguous(en_side) {
                lines.push(line.clone());
            }
        }
    }
    if lines.is_empty() {
        return String::new();
    }
    let mut ctx = String::from(
        "Translation glossary for context-dependent terms (choose the appropriate translation):\n",
    );
    for line in &lines {
        ctx.push_str(line);
        ctx.push('\n');
    }
    if !glossary.general_section().is_empty() {
        ctx.push('\n');
        ctx.push_str(glossary.general_section());
    }
    ctx
}

pub fn setup_deepl_glossary(
    client: &reqwest::blocking::Client,
    api_key: &str,
    glossary: &Glossary,
) -> Result<String, Box<dyn std::error::Error>> {
    // Build TSV entries only for unambiguous terms (single translation)
    let mut tsv = String::new();
    let mut seen_keys = std::collections::HashSet::new();
    let mut ambiguous_count = 0;
    for (_german, line) in &glossary.terms {
        if let Some(pos) = line.find(" = ") {
            let en_side = &line[pos + 3..];
            if is_ambiguous(en_side) {
                ambiguous_count += 1;
                continue; // Skip — will be passed as context instead
            }
            // Strip parenthetical annotations
            let clean = Regex::new(r"\s*\([^)]*\)\s*")
                .unwrap()
                .replace_all(en_side, " ")
                .trim()
                .to_string();
            if !clean.is_empty() {
                let de_original = line.split(" = ").next().unwrap().trim();
                let de_clean = if let Some(p) = de_original.find(" (") {
                    &de_original[..p]
                } else {
                    de_original
                };
                if seen_keys.insert(de_clean.to_string()) {
                    tsv.push_str(de_clean);
                    tsv.push('\t');
                    tsv.push_str(&clean);
                    tsv.push('\n');
                }
            }
        }
    }

    let entry_count = tsv.lines().count();
    eprintln!(
        "Creating DeepL glossary ({} unambiguous entries, {} ambiguous passed as context)...",
        entry_count, ambiguous_count
    );

    // Delete any existing glossaries first to avoid the limit
    if let Ok(resp) = client
        .get(format!("{}/v2/glossaries", deepl_api_base(api_key)))
        .header("Authorization", format!("DeepL-Auth-Key {}", api_key))
        .timeout(std::time::Duration::from_secs(10))
        .send()
    {
        if let Ok(json) = resp.json::<serde_json::Value>() {
            if let Some(glossaries) = json["glossaries"].as_array() {
                for g in glossaries {
                    if let Some(id) = g["glossary_id"].as_str() {
                        eprintln!("  Deleting old glossary {}...", id);
                        delete_deepl_glossary(client, api_key, id);
                    }
                }
            }
        }
    }

    let body = serde_json::json!({
        "name": "wittgenstein-nachlass",
        "source_lang": "de",
        "target_lang": "en",
        "entries": tsv,
        "entries_format": "tsv"
    });

    let resp = client
        .post(format!("{}/v2/glossaries", deepl_api_base(api_key)))
        .header("Authorization", format!("DeepL-Auth-Key {}", api_key))
        .json(&body)
        .timeout(std::time::Duration::from_secs(30))
        .send()?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        return Err(format!("DeepL glossary creation failed ({}): {}", status, text).into());
    }

    let json: serde_json::Value = resp.json()?;
    let id = json["glossary_id"]
        .as_str()
        .ok_or("Missing glossary_id in response")?
        .to_string();
    eprintln!("DeepL glossary created: {}", id);
    Ok(id)
}

/// Delete a DeepL glossary.
pub fn delete_deepl_glossary(
    client: &reqwest::blocking::Client,
    api_key: &str,
    glossary_id: &str,
) {
    let _ = client
        .delete(format!("{}/v2/glossaries/{}", deepl_api_base(api_key), glossary_id))
        .header("Authorization", format!("DeepL-Auth-Key {}", api_key))
        .timeout(std::time::Duration::from_secs(10))
        .send();
}

/// Call the DeepL API to translate text. Retries on 429 with exponential backoff.
pub fn call_deepl(
    client: &reqwest::blocking::Client,
    api_key: &str,
    text: &str,
    glossary_id: Option<&str>,
    context: Option<&str>,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut body = serde_json::json!({
        "text": [text],
        "source_lang": "DE",
        "target_lang": "EN-US",
        "tag_handling": "html",
    });

    if let Some(gid) = glossary_id {
        body["glossary_id"] = serde_json::Value::String(gid.to_string());
    }
    if let Some(ctx) = context {
        body["context"] = serde_json::Value::String(ctx.to_string());
    }

    for retry in 0..5 {
        if retry > 0 {
            let delay = std::time::Duration::from_secs(2u64.pow(retry as u32));
            eprint!(" (rate limited, waiting {}s)", delay.as_secs());
            std::thread::sleep(delay);
        }

        let resp = client
            .post(format!("{}/v2/translate", deepl_api_base(api_key)))
            .header("Authorization", format!("DeepL-Auth-Key {}", api_key))
            .json(&body)
            .timeout(std::time::Duration::from_secs(120))
            .send()?;

        if resp.status() == 429 {
            continue;
        }

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            return Err(format!("DeepL returned {}: {}", status, text).into());
        }

        let json: serde_json::Value = resp.json()?;
        let translated = json["translations"][0]["text"]
            .as_str()
            .ok_or("Missing translation in DeepL response")?
            .to_string();
        return Ok(translated);
    }
    Err("DeepL: rate limit exceeded after 5 retries".into())
}

/// Restart ollama by stopping and starting the service.
fn restart_ollama() {
    eprintln!("  Restarting ollama...");
    // Try pkill first (works on macOS and Linux)
    let _ = std::process::Command::new("pkill")
        .arg("-f")
        .arg("ollama")
        .status();
    std::thread::sleep(std::time::Duration::from_secs(3));
    // Start ollama serve in the background
    let _ = std::process::Command::new("ollama")
        .arg("serve")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    // Wait for it to be ready
    for _ in 0..30 {
        std::thread::sleep(std::time::Duration::from_secs(1));
        if std::net::TcpStream::connect("127.0.0.1:11434").is_ok() {
            eprintln!("  Ollama restarted.");
            return;
        }
    }
    eprintln!("  WARNING: ollama may not have restarted successfully.");
}

const MAX_OLLAMA_RETRIES: usize = 3;
const OLLAMA_TIMEOUT_SECS: u64 = 300;

/// Call ollama chat API with automatic restart on failure.
pub fn call_ollama(
    client: &reqwest::blocking::Client,
    ollama_url: &str,
    model: &str,
    system_msg: &str,
    user_msg: &str,
    num_ctx: usize,
) -> Result<String, Box<dyn std::error::Error>> {
    let body = json!({
        "model": model,
        "stream": false,
        "options": { "num_ctx": num_ctx },
        "messages": [
            { "role": "system", "content": system_msg },
            { "role": "user", "content": user_msg }
        ]
    });

    for attempt in 0..MAX_OLLAMA_RETRIES {
        let result = client
            .post(format!("{}/api/chat", ollama_url))
            .json(&body)
            .timeout(std::time::Duration::from_secs(OLLAMA_TIMEOUT_SECS))
            .send();

        match result {
            Ok(resp) => {
                if !resp.status().is_success() {
                    let status = resp.status();
                    if attempt < MAX_OLLAMA_RETRIES - 1 {
                        eprintln!(" ollama returned {}, restarting...", status);
                        restart_ollama();
                        continue;
                    }
                    return Err(format!("ollama returned {}", status).into());
                }
                let json: serde_json::Value = resp.json()?;
                return Ok(json["message"]["content"]
                    .as_str()
                    .unwrap_or("")
                    .to_string());
            }
            Err(e) => {
                if attempt < MAX_OLLAMA_RETRIES - 1 {
                    eprintln!(" request failed ({}), restarting ollama...", e);
                    restart_ollama();
                    continue;
                }
                return Err(e.into());
            }
        }
    }
    Err("ollama: max retries exceeded".into())
}

/// Build the translation system prompt with general glossary principles baked in.
pub fn translation_system_prompt(general_glossary: &str) -> String {
    let mut msg = format!(
        "Translate German to idiomatic English. Scholarly register, Wittgenstein specialist.\n\n\
         Rules:\n\
         1. Translate the ENTIRE input \u{2014} every sentence. Do not add content not in the original.\n\
         2. Preserve _underscores_ and **double asterisks** exactly. Same count in translation.\n\
         3. Copy HTML tags (<sup>, <s>, <em>, etc.) and \u{27E6}MATH:N\u{27E7} placeholders verbatim.\n\
         4. Use English curly quotes \u{201c}...\u{201d}, never ASCII \". Keep \"&\" for \"and\".\n\
         5. Do not translate proper names. Preserve all Unicode characters exactly.\n\
         6. Output only the translation.\n\n\
         Example:\n\
         Input: Er hat den 2<sup>ten</sup> \u{201e}Band\u{201c} gelesen & _wichtige_ Notizen \u{27E6}MATH:1\u{27E7} gemacht.\n\
         Output: He read the 2<sup>ten</sup> \u{201c}volume\u{201d} & made _important_ notes \u{27E6}MATH:1\u{27E7}."
    );
    if !general_glossary.is_empty() {
        msg.push_str("\n\n");
        msg.push_str(general_glossary);
    }
    msg
}

/// Build the user message for a translation request with context and filtered glossary.
pub fn translation_user_msg(
    text: &str,
    context: &[String],
    glossary_section: &str,
) -> String {
    let mut user_msg = String::new();
    if !glossary_section.is_empty() {
        user_msg.push_str(
            "Use the following glossary for established translations \
             of key philosophical terms. Follow these conventions strictly:\n\n",
        );
        user_msg.push_str(glossary_section);
        user_msg.push_str("\n\n");
    }
    let checklist = emphasis_checklist(text);
    if !checklist.is_empty() {
        user_msg.push_str(&checklist);
    }
    if !context.is_empty() {
        user_msg.push_str("Context (already translated preceding remarks):\n\n");
        for (i, ctx) in context.iter().enumerate() {
            if i > 0 {
                user_msg.push_str("\n\n---\n\n");
            }
            user_msg.push_str(ctx);
        }
        user_msg.push_str("\n\n---\n\nTranslate the following:\n\n");
    }
    user_msg.push_str(text);
    user_msg
}

// --- Index and work-assembly helpers ---

/// Parse index.md to get file ordering, split into docs and works.
pub fn parse_index_order(index_content: &str) -> (Vec<String>, Vec<String>) {
    let re = Regex::new(r"\]\(([^)]+\.md)\)").unwrap();
    let mut docs = Vec::new();
    let mut works = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for cap in re.captures_iter(index_content) {
        let filename = cap[1].to_string();
        if !seen.insert(filename.clone()) {
            continue;
        }
        if filename.starts_with("W-") {
            works.push(filename);
        } else {
            docs.push(filename);
        }
    }
    (docs, works)
}

/// Expand a list of top-level work files to include their split-work part files.
///
/// `all.md` links only the top-level work (e.g. `W-RFM.md`), but works split into
/// multiple parts keep their remarks in sibling files (`W-RFM-1.md`,
/// `W-RFM-1-App-1.md`, …). Each part must be assembled in its own right. We discover
/// them by filename: any `W-<root>-*.md` on disk belongs to the work rooted at
/// `W-<root>`. The top-level file is kept first (preserving `all.md` order); parts
/// follow in sorted order. Single-file works (no siblings) are returned unchanged.
pub fn expand_work_parts(works: &[String], input_dir: &Path) -> Vec<String> {
    let mut on_disk: Vec<String> = match fs::read_dir(input_dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.starts_with("W-") && n.ends_with(".md"))
            .collect(),
        Err(_) => Vec::new(),
    };
    on_disk.sort();

    let mut result = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for work in works {
        let prefix = format!("{}-", work.trim_end_matches(".md"));
        if seen.insert(work.clone()) {
            result.push(work.clone());
        }
        for name in &on_disk {
            if name.starts_with(&prefix) && seen.insert(name.clone()) {
                result.push(name.clone());
            }
        }
    }
    result
}

/// Extract the unique source document filenames referenced by a work file.
/// Work headings look like `### [Ms-172](/ms-172/#...): ...`
pub fn work_source_docs(work_path: &Path) -> Vec<String> {
    let content = fs::read_to_string(work_path).expect("Failed to read work file");
    let re = Regex::new(r"^### \[([A-Za-z]+-\d+[a-z]*)\]").unwrap();
    let mut seen = std::collections::HashSet::new();
    let mut docs = Vec::new();
    for line in content.lines() {
        if let Some(cap) = re.captures(line) {
            let name = format!("{}.md", &cap[1]);
            if seen.insert(name.clone()) {
                docs.push(name);
            }
        }
    }
    docs
}

/// Compute the anchor ID from a source doc remark heading's page refs.
/// E.g., heading `### [1\[1\]](url)` → `1.1`
pub fn anchor_from_doc_heading(heading: &str) -> String {
    // Strip HTML tags so links inside <span class="fac">...</span> are visible
    let no_html = Regex::new(r"<[^>]+>").unwrap().replace_all(heading, "");
    let link_re = Regex::new(r"\[([^\]]*(?:\\.[^\]]*)*)\]\(([^)]+)\)").unwrap();
    let mut parts = Vec::new();
    for cap in link_re.captures_iter(&no_html) {
        let url = &cap[2];
        // Skip work back-references (URLs like /w-pi/...)
        if url.starts_with("/w-") || url.contains("/w-") {
            continue;
        }
        let text = cap[1].replace("\\[", "[").replace("\\]", "]");
        parts.push(text);
    }
    let joined = parts.join(",");
    joined
        .split(',')
        .map(|seg| seg.replace('[', ".").replace(']', ""))
        .collect::<Vec<_>>()
        .join("+")
}

/// Extract doc name and anchor from a work remark heading.
/// E.g., `### [Ms-172](/ms-172/#1.1): ...` → `("Ms-172", "1.1")`
fn parse_work_heading(heading: &str) -> Option<(String, String)> {
    let re = Regex::new(r"^### \[([A-Za-z]+-\d+[a-z]*)\]\(/[^)]+/#([^)]+)\)").unwrap();
    re.captures(heading)
        .map(|cap| (cap[1].to_string(), cap[2].to_string()))
}

/// Build a lookup map from `DocName#anchor` → translated remark body,
/// reading all translated doc files in a directory.
pub fn build_remark_url_map(translated_dir: &Path, _input_dir: &Path) -> HashMap<String, String> {
    let mut map = HashMap::new();

    let entries: Vec<_> = fs::read_dir(translated_dir)
        .unwrap_or_else(|_| panic!("Cannot read {}", translated_dir.display()))
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.ends_with(".md") && !name.starts_with("W-") && name != "index.md"
        })
        .collect();

    for entry in entries {
        let path = entry.path();
        let doc_name = path.file_stem().unwrap().to_string_lossy().to_string();
        let content = fs::read_to_string(&path).expect("Failed to read translated file");
        let (_, remarks) = parse_document(&content);

        for remark in &remarks {
            let anchor = anchor_from_doc_heading(&remark.heading);
            if !anchor.is_empty() {
                let key = format!("{}#{}", doc_name, anchor);
                map.insert(key, remark.body.clone());
            }
        }
    }
    map
}

/// Assemble a translated work file from translated doc remarks.
pub fn assemble_work(
    work_german_path: &Path,
    url_map: &HashMap<String, String>,
    output_path: &Path,
) -> usize {
    let content = fs::read_to_string(work_german_path).expect("Failed to read work file");
    let (preamble, remarks) = parse_document(&content);
    let stem = work_german_path.file_name().unwrap().to_string_lossy();

    let mut missing = 0;
    let mut file = fs::File::create(output_path).expect("Failed to create work output file");
    write!(file, "{}", preamble).expect("Failed to write preamble");

    for remark in &remarks {
        let key = parse_work_heading(&remark.heading)
            .map(|(doc, anchor)| format!("{}#{}", doc, anchor))
            .unwrap_or_default();

        let translated_body = if let Some(doc_body) = url_map.get(&key) {
            // The German work is authoritative for series numbers: take the leading
            // series-number prefix from the work body, and strip *all* series numbers
            // from the translated doc body. Docs carry source-local numbering the
            // published work omits — both a leading prefix (e.g. Ms-144 `a*` paragraph
            // numbers, PI section numbers the work renumbers) and occasionally a
            // secondary mid-body number (e.g. Zettel 463 "Zur Mathematik" keeps a doc
            // paragraph number the work drops). Work remarks never carry a mid-body
            // series number, so stripping every span from the doc is safe.
            let lead_re = Regex::new(
                r#"^(<span class="series-number">[^<]+</span>\s*)+"#,
            )
            .unwrap();
            let any_re = Regex::new(
                r#"<span class="series-number">[^<]+</span>\s*"#,
            )
            .unwrap();
            let work_prefix = lead_re
                .find(&remark.body)
                .map(|m| m.as_str())
                .unwrap_or("");
            let doc_content = any_re.replace_all(doc_body, "");
            format!("{}{}", work_prefix, doc_content)
        } else {
            missing += 1;
            eprintln!(
                "  {} remark {}: no translated doc remark found, keeping German",
                stem,
                remark.heading.chars().take(60).collect::<String>()
            );
            remark.body.clone()
        };

        write!(file, "\n{}\n\n{}\n", remark.heading, translated_body)
            .expect("Failed to write remark");
    }

    missing
}

/// Write a single remark to a file (used by translate and fix).
pub fn write_remark(file: &mut fs::File, heading: &str, body: &str) {
    write!(file, "\n{}\n\n{}\n", heading, body).expect("Failed to write remark");
}

/// Get the last git commit hash that modified a file.
pub fn git_last_commit(repo_dir: &Path, file_path: &str) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["-C", &repo_dir.to_string_lossy(), "log", "-1", "--format=%H", "--", file_path])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if hash.is_empty() { None } else { Some(hash) }
}

/// Get file content at a specific git commit.
pub fn git_show(repo_dir: &Path, commit: &str, file_path: &str) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["-C", &repo_dir.to_string_lossy(), "show", &format!("{}:{}", commit, file_path)])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Detect remarks that changed between the old and current German text.
/// Returns a list of (remark_index, anchor_id) for remarks that need re-translation.
pub fn detect_changed_remarks(
    old_de_content: &str,
    current_de_content: &str,
) -> Vec<(usize, String)> {
    let (_, old_remarks) = parse_document(old_de_content);
    let (_, current_remarks) = parse_document(current_de_content);

    // Build map: anchor_id → body for old German
    let mut old_map: HashMap<String, String> = HashMap::new();
    for r in &old_remarks {
        let anchor = anchor_from_doc_heading(&r.heading);
        if !anchor.is_empty() {
            old_map.insert(anchor, r.body.clone());
        }
    }

    // Compare current remarks against old
    let mut changed = Vec::new();
    for (i, r) in current_remarks.iter().enumerate() {
        let anchor = anchor_from_doc_heading(&r.heading);
        if anchor.is_empty() {
            continue;
        }
        match old_map.get(&anchor) {
            Some(old_body) if old_body == &r.body => {} // unchanged
            _ => changed.push((i, anchor)),             // changed or new
        }
    }
    changed
}

/// Try to auto-fix a translation when only non-word prefixes or suffixes changed
/// between the old and new German text. If the English translation uses the same
/// prefix/suffix as the old German, substitute the new one. Returns the updated
/// English body, or None if the change requires retranslation.
pub fn try_auto_fix_remark(old_de: &str, new_de: &str, old_en: &str) -> Option<String> {
    if old_de == new_de {
        return None;
    }
    // Try prefix fix: find longest common tail, check if head is non-word
    let tail_len = common_byte_suffix_len(old_de, new_de);
    if tail_len > 0 {
        let old_head = &old_de[..old_de.len() - tail_len];
        let new_head = &new_de[..new_de.len() - tail_len];
        if is_non_word(old_head) && is_non_word(new_head) {
            if let Some(en_rest) = old_en.strip_prefix(old_head) {
                return Some(format!("{new_head}{en_rest}"));
            }
            // EN already has the new prefix (or no prefix if new_head is empty)
            if new_head.is_empty()
                || old_en.starts_with(new_head)
                || old_en.starts_with(new_head.trim_end())
            {
                return Some(old_en.to_string());
            }
        }
    }
    // Try suffix fix: find longest common head, check if tail is non-word
    let head_len = common_byte_prefix_len(old_de, new_de);
    if head_len > 0 {
        let old_tail = &old_de[head_len..];
        let new_tail = &new_de[head_len..];
        if is_non_word(old_tail) && is_non_word(new_tail) {
            if let Some(en_start) = old_en.strip_suffix(old_tail) {
                return Some(format!("{en_start}{new_tail}"));
            }
        }
    }
    None
}

/// Check if a string contains no alphabetic characters outside HTML tags.
fn is_non_word(s: &str) -> bool {
    let mut in_tag = false;
    for ch in s.chars() {
        if ch == '<' {
            in_tag = true;
        } else if ch == '>' {
            in_tag = false;
        } else if !in_tag && ch.is_alphabetic() {
            return false;
        }
    }
    true
}

fn common_byte_suffix_len(a: &str, b: &str) -> usize {
    let mut len = 0;
    for (ab, bb) in a.as_bytes().iter().rev().zip(b.as_bytes().iter().rev()) {
        if ab != bb {
            break;
        }
        len += 1;
    }
    // Ensure we land on a UTF-8 char boundary
    while len > 0 && !a.is_char_boundary(a.len() - len) {
        len -= 1;
    }
    len
}

fn common_byte_prefix_len(a: &str, b: &str) -> usize {
    let mut len = 0;
    for (ab, bb) in a.as_bytes().iter().zip(b.as_bytes().iter()) {
        if ab != bb {
            break;
        }
        len += 1;
    }
    while len > 0 && !a.is_char_boundary(len) {
        len -= 1;
    }
    len
}

/// Load the skip-remarks list. Returns a set of "filename:remark_id" strings.
pub fn load_skip_remarks(tool_dir: &Path) -> std::collections::HashSet<String> {
    let path = tool_dir.join("skip-remarks.txt");
    let mut set = std::collections::HashSet::new();
    if let Ok(content) = fs::read_to_string(&path) {
        for line in content.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() && !trimmed.starts_with('#') {
                set.insert(trimmed.to_string());
            }
        }
    }
    set
}

/// Check if a remark should be skipped (use original verbatim).
pub fn should_skip_remark(
    skip_set: &std::collections::HashSet<String>,
    filename: &str,
    remark_id: &str,
) -> bool {
    skip_set.contains(&format!("{}:{}", filename, remark_id))
}

/// Extract content words (>2 chars) from text, stripping HTML tags, markdown emphasis,
/// and non-alphanumeric characters. Used for fuzzy matching between document variants.
fn content_words(text: &str) -> Vec<String> {
    let no_html = Regex::new(r"<[^>]+>")
        .unwrap()
        .replace_all(text, " ")
        .into_owned();
    let clean = no_html.replace('_', " ").replace("**", " ").replace('*', " ");
    clean
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 2)
        .map(|w| w.to_lowercase())
        .collect()
}

/// First N content words as a lookup key.
fn word_key(text: &str, n: usize) -> String {
    content_words(text)
        .into_iter()
        .take(n)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Word-set overlap (Jaccard similarity).
fn word_similarity(a: &str, b: &str) -> f64 {
    let words_a: std::collections::HashSet<String> = content_words(a).into_iter().collect();
    let words_b: std::collections::HashSet<String> = content_words(b).into_iter().collect();
    let intersection = words_a.intersection(&words_b).count();
    let union = words_a.union(&words_b).count();
    if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    }
}

/// Find a translated sibling document for reuse.
/// E.g., for "Ts-227b.md", looks for translated "Ts-227a.md", "Ts-227c.md", etc.
pub fn find_translated_sibling(
    filename: &str,
    input_dir: &Path,
    output_dir: &Path,
) -> Option<String> {
    let stem = filename.strip_suffix(".md")?;
    let bytes = stem.as_bytes();
    let last = *bytes.last()?;
    if !last.is_ascii_lowercase() {
        return None;
    }
    let base = &stem[..stem.len() - 1];

    for letter in b'a'..=b'z' {
        if letter == last {
            continue;
        }
        let sibling = format!("{}{}.md", base, letter as char);
        let sibling_translated = output_dir.join(&sibling);
        let sibling_original = input_dir.join(&sibling);
        if sibling_translated.exists()
            && sibling_original.exists()
            && sibling_translated.extension().map_or(false, |e| e == "md")
        {
            return Some(sibling);
        }
    }
    None
}

struct ReuseEntry {
    full_de: String,
    en_body: String,
}

pub struct ReuseMap {
    /// Maps first-6-content-words → list of candidate entries
    entries: HashMap<String, Vec<ReuseEntry>>,
    pub len: usize,
}

impl ReuseMap {
    /// Look up a remark body. Returns the English translation if the German text
    /// matches a sibling remark with ≥85% word overlap.
    pub fn lookup(&self, german_body: &str) -> Option<&str> {
        let key = word_key(german_body, 6);
        if key.is_empty() {
            return None;
        }
        let candidates = self.entries.get(&key)?;
        for entry in candidates {
            if word_similarity(&entry.full_de, german_body) >= 0.85 {
                return Some(&entry.en_body);
            }
        }
        None
    }
}

/// Build a reuse map from a translated sibling.
pub fn build_reuse_map(german_path: &Path, english_path: &Path) -> ReuseMap {
    let de_content = fs::read_to_string(german_path).expect("Failed to read German sibling");
    let en_content = fs::read_to_string(english_path).expect("Failed to read English sibling");
    let (_, de_remarks) = parse_document(&de_content);
    let (_, en_remarks) = parse_document(&en_content);

    let mut entries: HashMap<String, Vec<ReuseEntry>> = HashMap::new();
    let mut count = 0;
    for (de, en) in de_remarks.iter().zip(en_remarks.iter()) {
        let key = word_key(&de.body, 6);
        if key.is_empty() {
            continue;
        }
        entries.entry(key).or_default().push(ReuseEntry {
            full_de: de.body.clone(),
            en_body: en.body.clone(),
        });
        count += 1;
    }
    ReuseMap { entries, len: count }
}
