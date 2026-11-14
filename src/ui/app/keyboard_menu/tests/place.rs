use super::file_place;
use crate::ui::date_groups::DisplayRow;
use crate::ui::scale::set_reference;
use crate::ui::style::{row_height, CardLayout, GridSize};

#[test]
fn finds_the_row_and_column_of_a_file() {
    set_reference(16);
    let rows = [DisplayRow::Files(vec![0, 1, 2]), DisplayRow::Files(vec![3, 4, 5])];
    assert_eq!(file_place(&rows, 4, Some(CardLayout::Grid(GridSize::Medium))), Some((CardLayout::Grid(GridSize::Medium).row_height(), CardLayout::Grid(GridSize::Medium).row_height(), 1)));
    assert_eq!(file_place(&rows, 0, None), Some((0.0, row_height(), 0)));
    assert_eq!(file_place(&rows, 9, None), None);
}
