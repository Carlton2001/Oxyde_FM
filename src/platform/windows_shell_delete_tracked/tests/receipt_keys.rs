use super::*;

// The Shell reports `\\srv\share\…` for a root canonicalized as
// `\\?\UNC\srv\share\…`: both must give the same key.
#[test]
fn network_roots_and_shell_paths_share_one_key() {
    assert_eq!(key(Path::new(r"\\?\UNC\srv\share\folder")), key(Path::new(r"\\srv\share\folder")));
    assert_eq!(key(Path::new(r"\\?\C:\folder")), key(Path::new(r"C:\folder")));
    assert_eq!(key(Path::new("C:/folder/file")), r"C:\folder\file");
}

// A node_modules past the receipt budget is deleted to the end, unrecorded
// past the budget, instead of being stopped halfway.
#[test]
fn descendants_past_the_budget_are_left_unrecorded_instead_of_stopped() {
    std::thread::spawn(|| {
        use std::os::windows::ffi::OsStrExt;
        let _apartment = super::super::Apartment::new().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let child = temp.path().join("child.txt");
        std::fs::write(&child, b"x").unwrap();
        let wide: Vec<_> = child.as_os_str().encode_wide().chain([0]).collect();
        let item: IShellItem = unsafe {
            windows::Win32::UI::Shell::SHCreateItemFromParsingName(windows::core::PCWSTR(wide.as_ptr()), None)
        }
        .unwrap();
        let mut effects = Effects::default();
        for index in 0..MAX_ITEMS {
            effects.pending.insert(format!("{index}"), (PathBuf::new(), None));
        }
        effects.pre(temp.path(), Ref::from(&item)).unwrap();
        assert_eq!(effects.untracked(), 1);
        assert_eq!(effects.pending.len(), MAX_ITEMS);
        effects.pending.clear();
        effects.pre(temp.path(), Ref::from(&item)).unwrap();
        assert_eq!(effects.pending.len(), 1);
    })
    .join()
    .unwrap();
}
