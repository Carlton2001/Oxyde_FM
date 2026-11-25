use super::next_row;

#[test]
fn arrows_move_within_the_shown_rows_and_stop_at_their_ends() {
    let rows = [3, 5, 8];
    assert_eq!(next_row(&rows, 5, "down"), Some((8, 2)));
    assert_eq!(next_row(&rows, 8, "down"), Some((8, 2)));
    assert_eq!(next_row(&rows, 5, "up"), Some((3, 0)));
    assert_eq!(next_row(&rows, 3, "up"), Some((3, 0)));
    assert_eq!(next_row(&rows, 5, "end"), Some((8, 2)));
    assert_eq!(next_row(&rows, 8, "home"), Some((3, 0)));
}

#[test]
fn a_hidden_selection_starts_from_the_first_row_and_an_empty_list_takes_nothing() {
    assert_eq!(next_row(&[3, 5], 4, "down"), Some((3, 0)));
    assert_eq!(next_row(&[], 0, "down"), None);
    assert_eq!(next_row(&[3], 3, "tab"), None);
}
