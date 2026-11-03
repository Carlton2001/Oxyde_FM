use super::{with_selection, Selection};
use gpui::{FontWeight, HighlightStyle};

#[test]
fn selection_covers_whole_middle_blocks_and_clamps_ends() {
    let selection = Selection { anchor: (3, 2), head: (1, 4) };
    assert_eq!(selection.within(0, 10), None);
    assert_eq!(selection.within(1, 10), Some(4..10));
    assert_eq!(selection.within(2, 7), Some(0..7));
    assert_eq!(selection.within(3, 10), Some(0..2));
    assert_eq!(selection.within(1, 3), None);
}

#[test]
fn selection_splits_styles_and_fills_gaps() {
    let bold = HighlightStyle { font_weight: Some(FontWeight::BOLD), ..Default::default() };
    let background = gpui::red();
    let result = with_selection(vec![(2..6, bold), (8..10, bold)], Some(4..9), background);
    let ranges: Vec<_> = result.iter().map(|(range, style)| (range.clone(), style.background_color.is_some())).collect();
    assert_eq!(ranges, vec![(2..4, false), (4..6, true), (6..8, true), (8..9, true), (9..10, false)]);
    assert_eq!(result[1].1.font_weight, Some(FontWeight::BOLD));
    assert_eq!(with_selection(vec![], Some(1..3), background), vec![(1..3, HighlightStyle { background_color: Some(background), ..Default::default() })]);
}
