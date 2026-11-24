use super::nearest_shown;
use std::collections::HashSet;

#[test]
fn an_arrow_from_a_folded_group_reaches_the_nearest_file_shown() {
    // Display order 0..6; files 2 and 3 are in a folded group.
    let order = [0, 1, 2, 3, 4, 5];
    let shown: HashSet<usize> = [0, 1, 4, 5].into_iter().collect();
    assert_eq!(nearest_shown(&order, &shown, 3, true), Some(4));
    assert_eq!(nearest_shown(&order, &shown, 2, false), Some(1));
    assert_eq!(nearest_shown(&order, &[0].into_iter().collect(), 3, true), None);
    assert_eq!(nearest_shown(&order, &shown, 9, true), None);
}
