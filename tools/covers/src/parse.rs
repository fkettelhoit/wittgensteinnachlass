use regex::Regex;

pub struct CoverData {
    pub title: String,
    pub subtitle: Option<String>,
    pub paragraphs: Vec<Paragraph>,
}

pub struct Paragraph {
    #[allow(dead_code)]
    pub text: String,
    pub len: usize,
    pub has_bold: bool,
    pub question_marks: usize,
    pub periods: usize,
}

pub fn parse_for_cover(content: &str) -> CoverData {
    let mut lines = content.lines();

    // Extract title from first "# " line
    let title = lines
        .by_ref()
        .find(|l| l.starts_with("# "))
        .map(|l| l[2..].trim().to_string())
        .unwrap_or_default();

    // For W-* files, extract year/info from <summary> tag
    let subtitle = extract_subtitle(content);

    // Collect paragraph text from remarks
    let paragraphs = extract_paragraphs(content);

    CoverData {
        title,
        subtitle,
        paragraphs,
    }
}

fn extract_subtitle(content: &str) -> Option<String> {
    let re = Regex::new(r"<summary>(.*?)</summary>").unwrap();
    re.captures(content).map(|c| c[1].trim().to_string())
}

fn extract_paragraphs(content: &str) -> Vec<Paragraph> {
    let mut paragraphs = Vec::new();
    let mut current = String::new();
    let mut in_details = false;
    let mut in_math_block = false;

    for line in content.lines() {
        // Track <details> blocks to skip them
        if line.contains("<details") {
            in_details = true;
            continue;
        }
        if line.contains("</details>") {
            in_details = false;
            continue;
        }
        if in_details {
            continue;
        }

        // Track multi-line <math display="block"> to skip
        if line.contains("<math") && line.contains("display=\"block\"") && !line.contains("</math>")
        {
            in_math_block = true;
            continue;
        }
        if in_math_block {
            if line.contains("</math>") {
                in_math_block = false;
            }
            continue;
        }

        // Skip headings, horizontal rules, and empty lines
        if line.starts_with('#') || is_horizontal_rule(line) || line.trim().is_empty() {
            // Flush current paragraph
            if !current.is_empty() {
                paragraphs.push(make_paragraph(&current));
                current.clear();
            }
            continue;
        }

        // Skip lines that are purely HTML block elements
        let trimmed = line.trim();
        if trimmed.starts_with("<img ")
            || trimmed.starts_with("<summary")
            || trimmed.starts_with("</summary")
            || trimmed == "<details>"
            || trimmed == "</details>"
        {
            continue;
        }

        // Accumulate paragraph text
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(trimmed);
    }

    if !current.is_empty() {
        paragraphs.push(make_paragraph(&current));
    }

    paragraphs
}

fn is_horizontal_rule(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.len() < 3 {
        return false;
    }
    let chars: Vec<char> = trimmed.chars().filter(|c| !c.is_whitespace()).collect();
    !chars.is_empty() && chars.iter().all(|&c| c == '-' || c == '*' || c == '_')
}

fn make_paragraph(text: &str) -> Paragraph {
    let question_marks = text.chars().filter(|&c| c == '?').count();
    let periods = text.chars().filter(|&c| c == '.').count();
    let has_bold = text.contains("**");
    let stripped = strip_markup(text);
    let len = stripped.len();
    Paragraph {
        text: stripped,
        len,
        has_bold,
        question_marks,
        periods,
    }
}

fn strip_markup(text: &str) -> String {
    let mut s = text.to_string();
    // Remove HTML tags
    let tag_re = Regex::new(r"<[^>]+>").unwrap();
    s = tag_re.replace_all(&s, "").to_string();
    // Remove markdown bold markers
    s = s.replace("**", "");
    // Remove markdown emphasis markers (simple: single underscores around words)
    let em_re = Regex::new(r"_([^_]+)_").unwrap();
    s = em_re.replace_all(&s, "$1").to_string();
    // Remove markdown links [text](url)
    let link_re = Regex::new(r"\[([^\]]*)\]\([^)]*\)").unwrap();
    s = link_re.replace_all(&s, "$1").to_string();
    // Remove {primary // alternative} variants — keep primary
    let variant_re = Regex::new(r"\{([^/}]*)\s*//[^}]*\}").unwrap();
    s = variant_re.replace_all(&s, "$1").to_string();
    s
}
