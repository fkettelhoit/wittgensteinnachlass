use regex::Regex;

fn anchor_from_doc_heading(heading: &str) -> String {
    // Extract the link text portions (page refs) from the heading
    let link_re = Regex::new(r"\[([^\]]*(?:\\.[^\]]*)*)\]\([^)]+\)").unwrap();
    let mut parts = Vec::new();
    for cap in link_re.captures_iter(heading) {
        let text = cap[1].replace("\\[", "[").replace("\\]", "]");
        parts.push(text);
    }
    let joined = parts.join(",");
    // Convert to anchor format: replace [ with ., drop ], join with +
    joined
        .split(',')
        .map(|seg| seg.replace('[', ".").replace(']', ""))
        .collect::<Vec<_>>()
        .join("+")
}

fn main() {
    // Test cases from source docs
    let test_cases = vec![
        ("### [113r\\[1\\]](url)", "113r.1"),
        ("### [113r\\[3\\]](url)", "113r.3"),
        ("### [113r\\[4\\]](url),[114r\\[1\\]](url)", "113r.4+114r.1"),
        ("### [21\\[1\\]](url)", "21.1"),
        ("### [21\\[5\\]](url),[22\\[1\\]](url)", "21.5+22.1"),
        ("### [1\\[1\\]](url)", "1.1"),
        ("### [Ir\\[1\\]](url)", "Ir.1"),
    ];

    for (heading, expected) in test_cases {
        let result = anchor_from_doc_heading(heading);
        let status = if result == expected { "OK" } else { "FAIL" };
        println!("{}: {} -> {} (expected {})", status, heading, result, expected);
    }
}
