use regex::Regex;

fn anchor_from_doc_heading(heading: &str) -> String {
    let link_re = Regex::new(r"\[([^\]]*(?:\\.[^\]]*)*)\]\([^)]+\)").unwrap();
    let mut parts = Vec::new();
    for cap in link_re.captures_iter(heading) {
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

fn main() {
    let test_cases = vec![
        ("### [113r\\[3\\]](url)", "113r.3"),
        ("### [113r\\[4\\]](url),[114r\\[1\\]](url)", "113r.4+114r.1"),
        ("### [21\\[1\\]](url)", "21.1"),
        ("### [1\\[1\\]](url)", "1.1"),
        ("### [1r\\[1\\]](url),[1v\\[1\\]](url),[2r\\[1\\]](url),[2v\\[1\\]](url),[3r\\[1\\]](url)", "1r.1+1v.1+2r.1+2v.1+3r.1"),
    ];
    
    for (heading, expected) in test_cases {
        let result = anchor_from_doc_heading(heading);
        let status = if result == expected { "OK" } else { "FAIL" };
        println!("{}: '{}' -> '{}' (expected '{}')", status, heading, result, expected);
    }
}
