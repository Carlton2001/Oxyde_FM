use super::{Item, PermissionMenu, PermissionsEdit, Pick};
use oxyde_lib::models::permissions::{Access, Account, Levels, PermissionClass, UnixPermissions};

fn item(name: &str, mode: u32, is_dir: bool, editable: bool) -> Item {
    let account = |id| Account { id, name: id.to_string() };
    Item {
        path: format!("/selection/{name}"),
        loaded: UnixPermissions {
            device: 1,
            inode: mode as u64,
            mode,
            is_dir,
            owner: account(1000),
            group: account(1000),
            groups: vec![account(1000), account(10)],
            owners: Vec::new(),
            editable,
        },
    }
}

fn modes(edit: &PermissionsEdit) -> Vec<(String, u32)> {
    edit.changes().into_iter().map(|(path, change)| (path, change.mode)).collect()
}

#[test]
fn a_selection_shows_what_its_items_share_and_mixed_otherwise() {
    let edit = PermissionsEdit::new(vec![item("a", 0o644, false, true), item("b", 0o600, false, true)], Levels::default()).unwrap();
    assert_eq!(edit.shown(false, PermissionClass::Owner, false), Some(0o644));
    assert_eq!(edit.shown(false, PermissionClass::Group, false), None);
    assert!(!edit.has_kind(true));
    assert!(edit.changes().is_empty());
}

#[test]
fn a_level_changes_its_class_on_every_item_of_its_kind_only() {
    let mut edit = PermissionsEdit::new(vec![
        item("a", 0o644, false, true),
        item("b", 0o600, false, true),
        item("dir", 0o755, true, true),
    ], Levels::default())
    .unwrap();
    edit.pick(PermissionMenu::Access { folders: false, class: PermissionClass::Group }, Pick::Access(Some(Access::ReadOnly)));
    // « a » already has it: only « b » changes, and no folder.
    assert_eq!(modes(&edit), [("/selection/b".into(), 0o640)]);
    // The « Mixed » choice gives each item its own bits back.
    edit.pick(PermissionMenu::Access { folders: false, class: PermissionClass::Group }, Pick::Access(None));
    assert!(edit.changes().is_empty());
}

#[test]
fn a_selection_keeps_each_owner_and_group_and_never_runs_files() {
    let mut edit = PermissionsEdit::new(vec![item("a", 0o644, false, true), item("b", 0o755, false, true)], Levels::default()).unwrap();
    assert!(edit.single().is_none());
    (edit.owner, edit.group) = (0, 10);
    assert!(edit.changes().is_empty());
}

#[test]
fn a_single_item_changes_its_group_and_execution() {
    let mut edit = PermissionsEdit::new(vec![item("script", 0o644, false, true)], Levels::default()).unwrap();
    edit.pick(PermissionMenu::Group, Pick::Group(10));
    edit.executable = Some(true);
    let changes = edit.changes();
    assert_eq!(changes.len(), 1);
    assert_eq!((changes[0].1.mode, changes[0].1.group, changes[0].1.owner), (0o755, 10, 1000));
}

#[test]
fn enclosed_levels_reach_folders_only_and_need_every_item_editable() {
    let mut edit = PermissionsEdit::new(vec![item("one", 0o755, true, true), item("two", 0o700, true, false)], Levels::default()).unwrap();
    assert!(edit.editable(false) && !edit.editable(true) && !edit.all_editable());
    // The page's levels apply only once « Change » confirmed them.
    edit.enclosed_draft.files[2] = Some(Access::None);
    assert!(edit.changes().is_empty());
    edit.enclosed = Some(edit.enclosed_draft.clone());
    assert_eq!(edit.changes().len(), 2);
    let mut edit = PermissionsEdit::new(vec![item("dir", 0o755, true, true), item("file", 0o644, false, true)], Levels::default()).unwrap();
    edit.enclosed = Some(Levels { files: [None, None, Some(Access::None)], ..Default::default() });
    assert_eq!(edit.changes().iter().map(|(path, _)| path.as_str()).collect::<Vec<_>>(), ["/selection/dir"]);
}
