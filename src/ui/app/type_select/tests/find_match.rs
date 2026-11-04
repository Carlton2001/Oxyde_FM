use super::find_match;

const NAMES: [&str; 5] = ["Alpha", "apple", "Beta", "banana", "Bob"];

#[test]
fn first_letter_selects_the_next_match_and_wraps() {
    assert_eq!(find_match(&NAMES, None, "a"), Some(0));
    assert_eq!(find_match(&NAMES, Some(0), "b"), Some(2));
    assert_eq!(find_match(&NAMES, Some(4), "a"), Some(0));
    assert_eq!(find_match(&NAMES, Some(0), "z"), None);
}

#[test]
fn longer_prefix_keeps_the_current_item_while_it_matches() {
    assert_eq!(find_match(&NAMES, Some(2), "ba"), Some(3));
    assert_eq!(find_match(&NAMES, Some(3), "ban"), Some(3));
    assert_eq!(find_match(&NAMES, Some(0), "al"), Some(0));
}

#[test]
fn repeated_letter_cycles_through_matches() {
    assert_eq!(find_match(&NAMES, Some(2), "bb"), Some(3));
    assert_eq!(find_match(&NAMES, Some(3), "bbb"), Some(4));
    assert_eq!(find_match(&NAMES, Some(4), "bbbb"), Some(2));
}

#[test]
fn repeated_letter_prefers_a_real_name() {
    let names = ["a", "aa.txt", "b"];
    assert_eq!(find_match(&names, None, "aa"), Some(1));
}
