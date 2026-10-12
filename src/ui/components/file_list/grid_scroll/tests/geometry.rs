use super::visible_rows;
use crate::ui::date_groups::{content_height, DateGroup, DisplayRow};

#[test]
fn final_grid_row_remains_visible_with_the_details_sized_empty_space() {
    crate::ui::scale::set_reference(16);
    let rows: Vec<_> = (0..100).map(|i| DisplayRow::Files(vec![i])).collect();
    let height = content_height(&rows, true);
    let viewport = 320.0;
    let gutter = crate::ui::style::row_height();
    let offset = height + gutter - viewport;
    let visible = visible_rows(&rows, false, offset, viewport);
    assert_eq!(visible.range.end, 100);
    assert_eq!(visible.bottom, height);
    assert!(gutter < crate::ui::style::grid_card_row_height());
    let represented = visible.top + content_height(&rows[visible.range.clone()], true) + height
        - visible.bottom
        + gutter;
    assert_eq!(represented, height + gutter);
}

#[test]
fn grouped_grid_keeps_header_heights_and_a_compact_final_gutter() {
    crate::ui::scale::set_reference(16);
    let rows = vec![
        DisplayRow::Header(DateGroup::Today, 2),
        DisplayRow::Files(vec![0, 1]),
        DisplayRow::Header(DateGroup::Older, 1),
        DisplayRow::Files(vec![2]),
    ];
    let height = content_height(&rows, true);
    let visible = visible_rows(
        &rows,
        true,
        height + crate::ui::style::row_height() - 100.0,
        100.0,
    );
    assert_eq!(visible.range.end, rows.len());
    assert_eq!(visible.bottom, height);
    assert_eq!(
        visible.top,
        content_height(&rows[..visible.range.start], true)
    );
}

#[test]
fn empty_grid_and_offsets_in_the_final_gutter_never_render_a_fake_file_row() {
    assert_eq!(visible_rows(&[], false, 0.0, 200.0).range, 0..0);
    let rows = vec![DisplayRow::Files(vec![0])];
    let visible = visible_rows(&rows, false, content_height(&rows, true) + 10.0, 200.0);
    assert_eq!(visible.range, 1..1);
}
