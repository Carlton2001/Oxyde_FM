use super::file_place;
use crate::ui::date_groups::DisplayRow;
use crate::ui::scale::set_reference;
use crate::ui::style::{grid_card_row_height, row_height};

#[test]
fn finds_the_row_and_column_of_a_file() {
    set_reference(16);
    let rows = [DisplayRow::Files(vec![0, 1, 2]), DisplayRow::Files(vec![3, 4, 5])];
    assert_eq!(file_place(&rows, 4, true), Some((grid_card_row_height(), grid_card_row_height(), 1)));
    assert_eq!(file_place(&rows, 0, false), Some((0.0, row_height(), 0)));
    assert_eq!(file_place(&rows, 9, false), None);
}
