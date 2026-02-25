fn main() {
    let content = r#"<w:p><w:r><w:t>Hello</w:t></w:r></w:p><w:p><w:r><w:t>World</w:t></w:r></w:p>"#;
    let mut stripped = String::with_capacity(content.len() / 2);
    let mut in_tag = false;
    let mut tag_buffer = String::new();
    for c in content.chars() {
        if c == '<' {
            in_tag = true;
            tag_buffer.clear();
        } else if c == '>' {
            in_tag = false;
            if tag_buffer.starts_with("w:p") || tag_buffer.starts_with("/w:p") || 
                tag_buffer.starts_with("w:br") || tag_buffer.starts_with("text:p") || 
                tag_buffer.starts_with("/text:p") || tag_buffer == "p" || tag_buffer == "/p" {
                stripped.push('\n');
            }
        } else if in_tag {
            if tag_buffer.len() < 10 {
                tag_buffer.push(c);
            }
        } else {
            stripped.push(c);
        }
    }
    println!("Stripped: {:?}", stripped);
}
