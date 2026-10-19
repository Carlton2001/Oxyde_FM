use super::*;

#[test]
fn headings_inline_styles_lists_and_fenced_code() {
    let (blocks, limited) = parse("# Title\n\nHello **bold** and *italic* with `code`.\n\n- first\n- second\n\n```rs\nlet x = 1;\n```", "C:/docs/readme.md");
    assert!(!limited);
    assert!(matches!(blocks[0].kind, Kind::Heading(1)));
    assert_eq!(blocks[0].text, "Title");
    assert!(blocks[1]
        .spans
        .iter()
        .any(|span| span.bold && &blocks[1].text[span.range.clone()] == "bold"));
    assert!(blocks[1].spans.iter().any(|span| span.italic));
    assert!(blocks[1].spans.iter().any(|span| span.code));
    assert!(blocks.iter().any(|block| block.text == "• first"));
    assert!(blocks
        .iter()
        .any(|block| matches!(block.kind, Kind::Code) && block.text.contains("let x")));
}

#[test]
fn local_images_resolve_relative_paths_and_remote_images_keep_alt_text() {
    let (blocks, _) = parse(
        "![été](images/my%20photo.png)\n\n![remote](https://example.org/photo.png)",
        "C:/docs/readme.md",
    );
    assert!(
        matches!(&blocks[0].kind, Kind::Image(Some(path)) if path.replace('\\', "/") == "C:/docs/images/my photo.png")
    );
    assert_eq!(blocks[0].text, "été");
    assert!(matches!(blocks[1].kind, Kind::Image(None)));
    assert_eq!(blocks[1].text, "remote");
    for destination in [
        "//server/image.png",
        "\\\\server\\image.png",
        "file:///C:/image.png",
        "data:image/png;base64,xx",
        "/image.png",
    ] {
        assert!(local_image("C:/docs/readme.md", destination).is_none());
    }
}

#[test]
fn raw_html_stays_literal_and_all_style_offsets_are_utf8_boundaries() {
    let (blocks, _) = parse(
        "**été** <script>test()</script>\n\n> Citation\n\n~~barré~~",
        "readme.md",
    );
    assert!(blocks.iter().any(|block| block.text.contains("<script>")));
    assert!(blocks.iter().any(|block| matches!(block.kind, Kind::Quote)));
    for block in blocks {
        for span in block.spans {
            assert!(span.range.end <= block.text.len());
            assert!(block.text.is_char_boundary(span.range.start));
            assert!(block.text.is_char_boundary(span.range.end));
        }
    }
}

#[test]
fn tables_and_large_blocks_have_bounded_native_layout() {
    let (blocks, _) = parse("| Name | Value |\n| --- | --- |\n| Alpha | Beta |", "readme.md");
    assert!(matches!(blocks[0].kind, Kind::TableRow(true)));
    assert!(matches!(blocks[1].kind, Kind::TableRow(false)));
    assert_eq!(blocks[1].text, "Alpha\tBeta\t");
    let (blocks, limited) = parse(&"é".repeat(20_000), "readme.md");
    assert!(limited);
    assert!(blocks.iter().all(|block| block.text.len() <= 16 * 1024));
}
