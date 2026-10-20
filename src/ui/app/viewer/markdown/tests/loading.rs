use super::Markdown;
use gpui::{px, ListOffset};

fn batch(count: usize) -> Vec<super::super::Block> {
    (0..count)
        .map(|index| super::super::Block {
            kind: super::super::Kind::Paragraph,
            text: index.to_string(),
            spans: Vec::new(),
        })
        .collect()
}

#[test]
fn appending_batches_keeps_the_initial_and_scrolled_anchor() {
    let mut document = Markdown::empty(false);
    document.append_batch(batch(32));
    assert_eq!(document.list.logical_scroll_top().item_ix, 0);
    document.list.scroll_to(ListOffset {
        item_ix: 12,
        offset_in_item: px(7.0),
    });
    document.append_batch(batch(32));
    let anchor = document.list.logical_scroll_top();
    assert_eq!(anchor.item_ix, 12);
    assert_eq!(anchor.offset_in_item, px(7.0));
    assert_eq!(document.blocks.borrow().len(), 64);
}
