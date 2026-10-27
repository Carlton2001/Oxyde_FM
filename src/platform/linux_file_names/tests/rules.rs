use super::*;
use std::os::unix::ffi::OsStrExt;

fn key(rules: FileNameRules, name: &str) -> OsString {
    rules.key(OsStr::new(name))
}

#[test]
fn an_ordinary_folder_keeps_names_apart() {
    let root = tempfile::tempdir().unwrap();
    let rules = rules(&File::open(root.path()).unwrap()).unwrap();
    assert_ne!(key(rules, "Notes"), key(rules, "notes"));
    assert_ne!(key(rules, "é"), key(rules, "e\u{301}"));
}

#[test]
fn caseless_systems_upcase_one_character_for_one() {
    let rules = FileNameRules::linux_caseless();
    assert_eq!(key(rules, "abc.txt"), key(rules, "ABC.txt"));
    assert_eq!(key(rules, "été.txt"), key(rules, "ÉTÉ.txt"));
    // « ß » has no one-character capital: it stays itself, as FAT and NTFS keep it.
    assert_ne!(key(rules, "straße"), key(rules, "STRASSE"));
    let invalid = OsStr::from_bytes(b"a\xff");
    assert_eq!(rules.key(invalid), invalid);
}

#[test]
fn casefold_ignores_case_and_normalization() {
    let rules = FileNameRules::linux_casefold();
    assert_eq!(key(rules, "Été"), key(rules, "e\u{301}te\u{301}"));
    assert_eq!(key(rules, "straße"), key(rules, "STRASSE"));
    assert_ne!(key(rules, "a"), key(rules, "b"));
}

/// Mounted vfat and exFAT images: `OXYDE_TEST_CASELESS=<mount>[:<mount>…]`.
#[test]
fn removable_media_ignore_case() {
    let Some(mounts) = std::env::var_os("OXYDE_TEST_CASELESS") else { return };
    for mount in std::env::split_paths(&mounts) {
        let rules = rules(&File::open(&mount).unwrap()).unwrap();
        assert_eq!(key(rules, "Été.txt"), key(rules, "été.TXT"), "{}", mount.display());
    }
}

/// An ext4 image made with `-O casefold`: `OXYDE_TEST_CASEFOLD=<folder with +F>:<folder without>`.
#[test]
fn only_a_casefold_folder_folds_on_ext4() {
    let Some(folders) = std::env::var_os("OXYDE_TEST_CASEFOLD") else { return };
    let folders: Vec<_> = std::env::split_paths(&folders).collect();
    let folded = rules(&File::open(&folders[0]).unwrap()).unwrap();
    assert_eq!(key(folded, "Été.txt"), key(folded, "e\u{301}te\u{301}.TXT"));
    let exact = rules(&File::open(&folders[1]).unwrap()).unwrap();
    assert_ne!(key(exact, "Été.txt"), key(exact, "été.txt"));
    // A held parent may be O_PATH: the flag is still read.
    let held = rustix::fs::open(&folders[0], OFlags::PATH | OFlags::DIRECTORY, Mode::empty()).unwrap();
    let folded = rules(&File::from(held)).unwrap();
    assert_eq!(key(folded, "A"), key(folded, "a"));
}
