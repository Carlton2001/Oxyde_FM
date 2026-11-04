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
    assert!(blocks.iter().any(|block| matches!(block.kind, Kind::Quote(None))));
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
    assert!(matches!(blocks[0].kind, Kind::TableRow(true, _)));
    assert!(matches!(blocks[1].kind, Kind::TableRow(false, _)));
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
        code.trim_end_matches('\n')
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

#[test]
fn html_code_elements_render_as_code() {
    let (blocks, _) = parse(
        "Press <kbd>Ctrl</kbd> then <code>a &lt; b</code> or <b>bold</b>.\n\n<pre><code>fn main() {\n    let x = 1 &lt; 2;\n}\n</code></pre>\n\nAfter",
        "C:/docs/readme.md",
    );
    let inline = &blocks[0];
    let code: String = inline
        .spans
        .iter()
        .filter(|span| span.code)
        .map(|span| &inline.text[span.range.clone()])
        .collect::<Vec<_>>()
        .concat();
    assert!(code.contains("Ctrl"));
    assert!(code.contains("a < b"));
    assert!(code.contains("<b>"));
    assert!(!inline.text.contains("<kbd>"));
    let block = blocks.iter().find(|block| matches!(block.kind, Kind::Code)).expect("code block");
    assert_eq!(block.text, "fn main() {\n    let x = 1 < 2;\n}");
    assert_eq!(blocks.last().unwrap().text, "After");
    assert_eq!(html_code_tag("<br/>"), None);
    assert_eq!(html_code_tag("</CODE>"), Some(false));
}


#[test]
fn task_items_replace_the_bullet_with_their_box() {
    let (blocks, _) = parse("- [ ] todo\n- [x] done\n  - [ ] nested\n- plain", "C:/docs/readme.md");
    let texts: Vec<_> = blocks.iter().map(|block| block.text.as_str()).collect();
    assert!(texts.contains(&"☐ todo"));
    assert!(texts.contains(&"☑ done"));
    assert!(texts.contains(&"  ☐ nested"));
    assert!(texts.contains(&"• plain"));
}

#[test]
fn plain_text_addresses_become_links_but_not_inside_code() {
    let (blocks, _) = parse("Visit www.example.com. Or `www.code.org`.", "C:/docs/readme.md");
    let block = &blocks[0];
    let linked: Vec<_> = block.spans.iter().filter(|span| span.link).map(|span| &block.text[span.range.clone()]).collect();
    assert_eq!(linked, ["www.example.com"]);
}

#[test]
fn alerts_get_a_title_and_footnotes_close_the_document() {
    let (blocks, _) = parse("> [!WARNING]\n> Careful.\n\nText[^b] and[^a].\n\n[^a]: First.\n\n[^b]: Second.\n\nEnd.", "C:/docs/readme.md");
    assert!(matches!(blocks[0].kind, Kind::AlertTitle(BlockQuoteKind::Warning)));
    assert!(matches!(blocks[1].kind, Kind::Quote(Some(BlockQuoteKind::Warning))) && blocks[1].text == "Careful.");
    assert_eq!(blocks[2].text, "Text[1] and[2].");
    assert_eq!(blocks[3].text, "End.");
    assert!(matches!(blocks[4].kind, Kind::Rule));
    assert_eq!(blocks[5].text, "1. Second.");
    assert_eq!(blocks[6].text, "2. First.");
}

#[test]
fn table_rows_carry_their_column_alignment() {
    let (blocks, _) = parse("| a | b | c |\n|:--|:-:|--:|\n| 1 | 2 | 3 |", "C:/docs/readme.md");
    assert!(matches!(&blocks[1].kind, Kind::TableRow(false, alignments)
        if alignments[..] == [Alignment::Left, Alignment::Center, Alignment::Right]));
}
