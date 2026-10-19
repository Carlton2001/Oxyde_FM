use super::{parse_document, BlockKind};

#[test]
fn document_lines_become_typed_blocks() {
    let blocks = parse_document("h1\tTitle\np\tBody\nli\tItem\ntr\ta\u{1f}b\nsheet\tData\nbad line\n");
    let kinds: Vec<_> = blocks.iter().map(|block| block.kind).collect();
    assert!(kinds == [BlockKind::Heading(1), BlockKind::Paragraph, BlockKind::Item, BlockKind::Row, BlockKind::Sheet]);
    assert_eq!(blocks[3].cells.len(), 2);
}
