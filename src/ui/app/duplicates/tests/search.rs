use super::contains_folded;

#[test]
fn folded_search_matches_ascii_and_other_paths_alike() {
    assert!(contains_folded("C:/Photos/Holiday.JPG", "holiday.jpg"));
    assert!(contains_folded("C:/Photos/Holiday.JPG", "c:/photos"));
    assert!(!contains_folded("C:/Photos/Holiday.JPG", "holidays"));
    assert!(!contains_folded("ab", "abc"));
    assert!(contains_folded("C:/Été/Ñandú.png", "été/ñandú"));
    assert!(contains_folded("C:/Été/Readme.TXT", "readme"));
    assert!(!contains_folded("C:/Été/Readme.TXT", "été/z"));
}
