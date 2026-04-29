use regex::Regex;

fn main() {
    let link_re = Regex::new(r"\[([^\]]*(?:\\.[^\]]*)*)\]\([^)]+\)").unwrap();
    
    let test_cases = vec![
        "### [113r\\[3\\]](url)",
        "### [113r\\[4\\]](url),[114r\\[1\\]](url)",
        "### [21\\[1\\]](url)",
        "### [1\\[1\\]](url)",
    ];
    
    for heading in test_cases {
        println!("Testing: {}", heading);
        for cap in link_re.captures_iter(heading) {
            println!("  Captured: '{}'" , &cap[1]);
            let text = cap[1].replace("\\[", "[").replace("\\]", "]");
            println!("  After unescape: '{}'", text);
        }
    }
}
