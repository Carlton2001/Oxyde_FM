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
    let (blocks, _) = parse(
        "| Name | Value |\n| --- | --- |\n| Alpha | Beta |",
        "readme.md",
    );
    assert!(matches!(blocks[0].kind, Kind::TableRow(true)));
    assert!(matches!(blocks[1].kind, Kind::TableRow(false)));
    assert_eq!(blocks[1].text, "Alpha\tBeta\t");
    let (blocks, limited) = parse(&"é".repeat(20_000), "readme.md");
    assert!(!limited);
    assert!(blocks.iter().all(|block| block.text.len() <= 16 * 1024));
    assert_eq!(
        blocks
            .iter()
            .map(|block| block.text.as_str())
            .collect::<String>(),
        "é".repeat(20_000)
    );
}

#[test]
fn progressive_batches_preserve_large_styled_blocks_and_fences() {
    let paragraph = "été 🙂 ".repeat(12_000).trim_end().to_owned();
    let code = "let valeur = 42;\n".repeat(4_000);
    let source = format!("**{paragraph}**\n\n```rs\n{code}```\n\n# Fin");
    let mut batches = Vec::new();
    parse_progressively(&source, "readme.md", &|| false, &mut |batch| {
        batches.push(batch);
        true
    });
    assert_eq!(batches[0].len(), 1);
    assert!(batches.iter().all(|batch| batch.len() <= 32));
    let blocks = batches.into_iter().flatten().collect::<Vec<_>>();
    assert_eq!(
        blocks
            .iter()
            .filter(|block| matches!(block.kind, Kind::Paragraph))
            .map(|block| block.text.as_str())
            .collect::<String>(),
        paragraph
    );
    assert_eq!(
        blocks
            .iter()
            .filter(|block| matches!(block.kind, Kind::Code))
            .map(|block| block.text.as_str())
            .collect::<String>(),
        code
    );
    assert_eq!(blocks.last().unwrap().text, "Fin");
    for block in &blocks {
        assert!(block.text.len() <= 16 * 1024);
        for span in &block.spans {
            assert!(block.text.is_char_boundary(span.range.start));
            assert!(block.text.is_char_boundary(span.range.end));
            if matches!(block.kind, Kind::Paragraph) {
                assert!(span.bold);
            }
        }
    }
}

#[test]
fn long_document_passes_old_byte_event_and_block_limits() {
    let source = format!(
        "{}\n# LAST",
        format!("{} **bold**\n\n", "x".repeat(90)).repeat(25_000)
    );
    assert!(source.len() > 2 * 1024 * 1024);
    let mut count = 0;
    let mut last = String::new();
    parse_progressively(&source, "readme.md", &|| false, &mut |batch| {
        count += batch.len();
        last = batch.last().unwrap().text.clone();
        true
    });
    assert_eq!(count, 25_001);
    assert_eq!(last, "LAST");
}

#[test]
fn publication_can_stop_and_cancellation_suppresses_batches() {
    let source = "paragraph\n\n".repeat(1000);
    let mut calls = 0;
    parse_progressively(&source, "readme.md", &|| false, &mut |_| {
        calls += 1;
        false
    });
    assert_eq!(calls, 1);
    parse_progressively(&source, "readme.md", &|| true, &mut |_| panic!("cancelled"));
}

#[test]
fn reference_links_and_ordered_lists_keep_context_across_batches() {
    let source = format!(
        "{}\n[link][target]\n\n[target]: https://example.org",
        (1..=100)
            .map(|n| format!("{n}. item\n"))
            .collect::<String>()
    );
    let (blocks, _) = parse(&source, "readme.md");
    assert!(blocks.iter().any(|block| block.text == "100. item"));
    let link = blocks.last().unwrap();
    assert_eq!(link.text, "link");
    assert!(link.spans.iter().any(|span| span.link));
}
