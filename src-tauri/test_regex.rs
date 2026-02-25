use regex::Regex;

fn main() {
    let xml = r#"<w:p><w:r><w:t>Hello</w:t></w:r><w:r><w:t xml:space="preserve"> world</w:t></w:r></w:p>"#;
    let mut in_tag = false;
    let stripped: String = xml.chars().filter(|&c| {
        if c == '<' { in_tag = true; false }
        else if c == '>' { in_tag = false; false }
        else { !in_tag }
    }).collect();

    println!("Stripped: '{}'", stripped);
    let r = Regex::new("(?i)hello.*world").unwrap();
    println!("Matches: {}", r.is_match(&stripped));
}
