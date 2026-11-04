use super::is_visible;

#[test]
fn system_takes_precedence_in_all_attribute_and_preference_combinations() {
    for hidden in [false, true] {
        for system in [false, true] {
            for show_hidden in [false, true] {
                for show_system in [false, true] {
                    let expected = match (hidden, system) {
                        (_, true) => show_system,
                        (true, false) => show_hidden,
                        (false, false) => true,
                    };
                    assert_eq!(
                        is_visible(hidden, system, show_hidden, show_system),
                        expected
                    );
                }
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
#[test]
fn linux_hides_dot_backup_and_listed_names_like_nautilus() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["plain.txt", ".dot", "notes.txt~", "listed"] {
        std::fs::write(dir.path().join(name), "").unwrap();
    }
    std::fs::write(dir.path().join(".hidden"), "listed\n").unwrap();
    let attributes = super::DirectoryAttributes::load(dir.path());
    let hidden = |name: &str| attributes.of(&std::fs::metadata(dir.path().join(name)).unwrap(), name).0;
    assert!(!hidden("plain.txt"));
    assert!(hidden(".dot") && hidden("notes.txt~") && hidden("listed"));
}
