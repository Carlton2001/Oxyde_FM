use super::selection;
#[test]
fn selection_wraps_and_survives_a_shorter_menu() {
    assert_eq!(selection(None, 3, "down"), Some(0));
    assert_eq!(selection(None, 3, "up"), Some(2));
    assert_eq!(selection(Some(2), 3, "down"), Some(0));
    assert_eq!(selection(Some(0), 3, "up"), Some(2));
    assert_eq!(selection(Some(8), 2, "down"), Some(0));
    assert_eq!(selection(Some(1), 0, "end"), None);
    assert_eq!(selection(Some(1), 3, "home"), Some(0));
    assert_eq!(selection(None, 3, "end"), Some(2));
}
