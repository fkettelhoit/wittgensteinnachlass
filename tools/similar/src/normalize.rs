//! Turn a remark's Markdown body into a list of normalized word tokens.

use regex::{Captures, Regex};
use std::sync::LazyLock;

macro_rules! re {
    ($name:ident, $pat:expr) => {
        static $name: LazyLock<Regex> = LazyLock::new(|| Regex::new($pat).unwrap());
    };
}

// Whole lines that carry no remark text: section titles, rules, dates.
re!(SKIP_LINE, r"^(##\s.*|\s*(-{3,}|(–\s*){3,})\s*|\s*\d{1,2}\.\d{1,2}\.\d{2,4}\.?\s*)$");
re!(MATH, r"(?s)<math\b.*?</math>");
re!(IMAGE, r"!\[[^\]]*\]\([^)]*\)");
re!(SERIES_NUMBER, r#"(?s)<span class="series-number">.*?</span>"#);
// A deletion; an unclosed `<s>` runs to the end of the remark.
re!(STRIKE, r"(?s)<s>.*?(</s>|\z)");
re!(TAG, r"<[^>]+>");
re!(LINK, r"\[([^\]]*)\]\([^)]*\)");
// Dotted abbreviations made of single letters: `d.h.`, `z.B.`, `u.s.w.` → `dh`, `zb`, `usw`.
re!(ABBREV, r"\b(?:\p{L}\.){2,}");
re!(INNER_HYPHEN, r"(\p{L})[-‐](\p{L})");
re!(TOKEN, r"[\p{L}\p{N}]+");

/// If dropping deletions would remove more than this share of a remark's tokens, the
/// deletion is kept: Wittgenstein often struck whole remarks after copying them, and those
/// are exactly the remarks that have other versions.
const MAX_DELETED_SHARE: f64 = 0.5;

pub fn tokens(body: &str) -> Vec<String> {
    let text: String = body
        .lines()
        .filter(|l| !SKIP_LINE.is_match(l))
        .collect::<Vec<_>>()
        .join("\n");
    let text = MATH.replace_all(&text, " ");
    let text = IMAGE.replace_all(&text, " ");
    let text = SERIES_NUMBER.replace_all(&text, " ");

    let with_deletions = tokenize(&text);
    let without_deletions = tokenize(&STRIKE.replace_all(&text, " "));
    let deleted = with_deletions.len() - without_deletions.len().min(with_deletions.len());
    if (deleted as f64) > MAX_DELETED_SHARE * with_deletions.len() as f64 {
        with_deletions
    } else {
        without_deletions
    }
}

fn tokenize(text: &str) -> Vec<String> {
    let text = TAG.replace_all(text, " ");
    let text = LINK.replace_all(&text, "$1");
    let text = text
        .to_lowercase()
        .replace('ß', "ss")
        .replace('&', " und ")
        .replace(['*', '_'], "");
    let text = ABBREV.replace_all(&text, |c: &Captures| {
        c[0].chars().filter(|ch| ch.is_alphabetic()).collect::<String>() + " "
    });
    // Twice, because matches can't overlap (`a-b-c`).
    let text = INNER_HYPHEN.replace_all(&text, "$1$2");
    let text = INNER_HYPHEN.replace_all(&text, "$1$2");
    TOKEN
        .find_iter(&text)
        // `&` also stands for "and" in the English remarks.
        .map(|m| match m.as_str() {
            "and" => "und".to_string(),
            t => t.to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::tokens;

    fn t(s: &str) -> String {
        tokens(s).join(" ")
    }

    #[test]
    fn und_and_eszett() {
        assert_eq!(t("Häusern & Bäumen, daß"), "häusern und bäumen dass");
        assert_eq!(t("this & not that and"), "this und not that und");
    }

    #[test]
    fn abbreviations() {
        assert_eq!(t("d.h. z.B. u.s.w. usw. etc."), "dh zb usw usw etc");
    }

    #[test]
    fn markup() {
        assert_eq!(
            t("<span class=\"series-number\">12</span> Das **ist** _so_ ![](x.svg) <math display=\"inline\"><mi>p</mi></math> Satz-Zeichen"),
            "das ist so satzzeichen"
        );
    }

    #[test]
    fn skipped_lines() {
        assert_eq!(t("14.12.1933\n\n## Titel\n\nText\n\n---\n"), "text");
    }

    #[test]
    fn small_deletion_dropped() {
        assert_eq!(t("Das ist <s>nicht</s> ein Satz der lang ist."), "das ist ein satz der lang ist");
    }

    #[test]
    fn whole_remark_deletion_kept() {
        assert_eq!(t("<s> Der Anblick</s> <s>meines Zimmers.</s>"), "der anblick meines zimmers");
    }
}
