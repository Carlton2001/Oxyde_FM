use super::*;

#[test]
fn a_few_unreadable_entries_are_skipped_and_too_many_fail_the_folder() {
    let mut unreadable = 0;
    for _ in 0..MAX_UNREADABLE_ENTRIES {
        assert!(skip_unreadable_entry(&mut unreadable, "dir", std::io::Error::other("bad")).is_ok());
    }
    assert!(skip_unreadable_entry(&mut unreadable, "dir", std::io::Error::other("bad")).is_err());
}
