use super::DetailsColumnWidths;
use crate::ui::scale::{logical, set_reference};

#[test]
fn size_column_keeps_measured_width_in_narrow_and_search_panes() {
    for reference in [10, 16, 24, 32] {
        set_reference(reference);
        for search in [false, true] {
            for width in [320.0, 600.0, 1000.0] {
                let columns = DetailsColumnWidths::for_viewport_with_search(
                    logical(width), 140.0, 120.0, 180.0, search,
                );
                assert_eq!(columns.size, logical(120.0));
                assert_eq!(columns.date, logical(180.0));
                assert!(columns.name >= logical(220.0));
                if width == 320.0 {
                    assert!(columns.content_width() > logical(width));
                }
            }
        }
    }
    set_reference(16);
}
