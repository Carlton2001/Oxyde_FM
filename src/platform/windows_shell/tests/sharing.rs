#[test]
#[ignore = "Requires an interactive Windows session with network sharing enabled"]
fn sharing_submenu_contains_windows_commands() {
    let folder = tempfile::tempdir().unwrap();
    let file = folder.path().join("sharing-test.txt");
    std::fs::write(&file, b"Oxyde sharing menu test").unwrap();
    for path in [folder.path(), &file] {
        let items = crate::platform::platform()
            .native_context_menu_items(&[path.to_str().unwrap().to_owned()], false)
            .unwrap();
        let sharing = items
            .iter()
            .find(|item| item.has_submenu && item.verb.as_deref() == Some(SHARING_VERB))
            .expect("Windows must offer its sharing submenu for the test selection");
        assert!(sharing.children.iter().filter(|child| child.id > 0).count() >= 2);
        assert!(sharing
            .children
            .iter()
            .all(|child| !child.is_folder_sharing_command));
    }
}

#[test]
#[ignore = "Requires an interactive Windows session with network sharing enabled"]
fn folder_background_resolves_sharing_in_the_displayed_folder_menu() {
    let folder = tempfile::tempdir().unwrap();
    let paths = [folder.path().to_str().unwrap().to_owned()];
    let platform = crate::platform::platform();
    let items = platform.native_context_menu_items(&paths, true).unwrap();
    assert!(items
        .iter()
        .all(|item| !item.has_submenu || !item.children.is_empty()));
    let sharing = items
        .iter()
        .find(|item| item.verb.as_deref() == Some(SHARING_VERB))
        .unwrap();
    let folder_items = platform.native_context_menu_items(&paths, false).unwrap();
    let native_sharing = folder_items
        .iter()
        .find(|item| item.verb.as_deref() == Some(SHARING_VERB))
        .unwrap();
    assert!(sharing.children.len() >= 2);
    assert_eq!(sharing.children.len(), native_sharing.children.len());
    for (shown, native) in sharing.children.iter().zip(&native_sharing.children) {
        assert_eq!(shown.label, native.label);
        assert!(shown.is_folder_sharing_command);
        let mut picked = shown.clone();
        picked.id = 0; // The ID from the snapshot must not drive the invocation.
        let resolved = with_folder_sharing_command(&paths, &picked, |_, id| Ok(id)).unwrap();
        assert!((1..=0x7FFF).contains(&resolved));
    }

    let file = folder.path().join("replacement.txt");
    std::fs::write(&file, b"Oxyde sharing menu test").unwrap();
    assert!(platform
        .execute_native_context_menu_item(
            &[file.to_str().unwrap().into()],
            &sharing.children[0],
            true
        )
        .is_err());
}
use super::*;

fn command(id: i32, label: &str) -> WinMenuItem {
    WinMenuItem {
        id,
        label: label.into(),
        verb: None,
        has_submenu: false,
        is_folder_sharing_command: true,
        children: Vec::new(),
    }
}

#[test]
fn resolves_sharing_by_command_identity_and_rejects_ambiguity() {
    let picked = command(4, "Specific people");
    let fresh = command(22, "Specific people");
    let other = command(4, "Remove access");
    assert_eq!(
        matching_command_id(&[other.clone(), fresh.clone()], &picked),
        Some(22)
    );
    assert_eq!(matching_command_id(&[other], &picked), None);
    assert_eq!(
        matching_command_id(&[fresh, command(33, "Specific people")], &picked),
        None
    );
}
