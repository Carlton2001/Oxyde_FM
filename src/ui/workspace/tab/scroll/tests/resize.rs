use super::{display_rows, rem_pixels, row_height, DisplayRow, TabState, ViewMode};
use gpui::{point, px, rems, size};
use oxyde_lib::models::FileEntry;

fn fixture(mode: ViewMode) -> TabState {
    crate::ui::scale::set_reference(16);
    let mut tab = TabState::new("test".into(), "folder".into());
    tab.view_mode = mode;
    tab.files = (0..100)
        .map(|index| FileEntry {
            name: format!("file-{index:03}"),
            path: format!("folder/file-{index:03}"),
            modified: 0,
            ..Default::default()
        })
        .collect();
    tab.update_cache();
    tab.keyboard_cursor = Some("folder/file-090".into());
    tab
}

#[test]
fn shrinking_details_pane_keeps_focused_row_above_status_bar() {
    let mut tab = fixture(ViewMode::Details);
    tab.keep_cursor_visible(size(800.0, 300.0));
    let bottom = 91.0 * crate::ui::style::row_height();
    let visible_height = 300.0
        - rem_pixels(crate::ui::style::TAB_BAR_HEIGHT).as_f32()
        - rem_pixels(crate::ui::style::NAV_BAR_HEIGHT).as_f32()
        - rem_pixels(rems(2.0)).as_f32()
        - crate::ui::style::column_header_height()
        - crate::ui::components::scrollbar::zed_track_width();
    let visible_top = -tab.list_scroll.0.borrow().base_handle.offset().y.as_f32();
    assert!((visible_top + visible_height - bottom).abs() < 0.1);
}

#[test]
fn resizing_without_focus_preserves_scroll_position() {
    let mut tab = fixture(ViewMode::Grid);
    tab.keyboard_cursor = None;
    tab.list_scroll
        .0
        .borrow()
        .base_handle
        .set_offset(point(px(0.0), px(-150.0)));
    tab.keep_cursor_visible(size(800.0, 300.0));
    assert_eq!(
        tab.list_scroll.0.borrow().base_handle.offset().y,
        px(-150.0)
    );
}

#[test]
fn shrinking_grouped_grid_reveals_focus_with_variable_row_heights() {
    let mut tab = fixture(ViewMode::Grid);
    tab.group_by_date = true;
    tab.keep_cursor_visible(size(800.0, 300.0));
    let columns = crate::ui::style::GridGeometry::for_viewport(
        800.0,
        crate::ui::components::scrollbar::zed_track_width(),
    )
    .columns;
    let rows = display_rows(
        tab.filtered_files(),
        true,
        &tab.collapsed_date_groups,
        columns,
    );
    let row = rows
        .iter()
        .position(|row| matches!(row, DisplayRow::Files(indices) if indices.contains(&90)))
        .unwrap();
    let top = crate::ui::date_groups::content_height(&rows[..row], true);
    let bottom = top + row_height(&rows[row], true);
    let viewport = 300.0
        - rem_pixels(crate::ui::style::TAB_BAR_HEIGHT).as_f32()
        - rem_pixels(crate::ui::style::NAV_BAR_HEIGHT).as_f32()
        - rem_pixels(rems(2.0)).as_f32();
    let visible_top = -tab.list_scroll.0.borrow().base_handle.offset().y.as_f32();
    assert!(top >= visible_top - 0.1 && bottom <= visible_top + viewport + 0.1);
}
