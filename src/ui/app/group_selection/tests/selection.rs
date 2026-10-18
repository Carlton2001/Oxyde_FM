use super::*;
use crate::ui::date_groups::{DisplayRow, FileFamily};
use std::sync::Arc;

fn fixture() -> TabState {
    let mut tab = TabState::new("tab".into(), "C:/".into());
    tab.group_mode = GroupMode::Type;
    tab.files = Arc::new(
        ["a.zip", "b.7z", "photo.png", "note.txt"]
            .map(|name| FileEntry {
                name: name.into(),
                path: format!("C:/{name}"),
                ..Default::default()
            })
            .to_vec(),
    );
    tab.update_cache();
    tab
}
fn state(tab: &TabState, group: &GroupKey) -> GroupSelection {
    let presentation = tab.presentation(false, 1);
    GroupSelection::for_members(
        &presentation.files,
        &presentation.groups[group],
        &tab.selected_paths,
    )
}

#[test]
fn group_checkboxes_accumulate_and_only_remove_their_own_members() {
    let mut tab = fixture();
    let archives = GroupKey::Family(FileFamily::Archives);
    let images = GroupKey::Family(FileFamily::Images);
    assert_eq!(state(&tab, &archives), GroupSelection::None);
    toggle_group(&mut tab, &archives);
    assert_eq!(tab.selected_paths.len(), 2);
    assert_eq!(state(&tab, &archives), GroupSelection::All);
    toggle_group(&mut tab, &images);
    assert_eq!(tab.selected_paths.len(), 3);
    assert_eq!(state(&tab, &archives), GroupSelection::All);
    toggle_group(&mut tab, &archives);
    assert_eq!(tab.selected_paths, HashSet::from(["C:/photo.png".into()]));
    assert_eq!(state(&tab, &images), GroupSelection::All);
    assert_eq!(state(&tab, &archives), GroupSelection::None);
}

#[test]
fn individual_refinement_produces_partial_state_then_group_click_completes_it() {
    let mut tab = fixture();
    let archives = GroupKey::Family(FileFamily::Archives);
    toggle_group(&mut tab, &archives);
    // Individual Ctrl-click changes the same selected_paths set.
    tab.selected_paths.remove("C:/b.7z");
    tab.selected_paths.insert("C:/note.txt".into());
    assert_eq!(state(&tab, &archives), GroupSelection::Partial);
    toggle_group(&mut tab, &archives);
    assert_eq!(state(&tab, &archives), GroupSelection::All);
    assert!(tab.selected_paths.contains("C:/note.txt"));
    toggle_group(&mut tab, &archives);
    assert_eq!(tab.selected_paths, HashSet::from(["C:/note.txt".into()]));
    assert_eq!(tab.selection_anchor, None);
    assert_eq!(tab.keyboard_cursor, None);
}

#[test]
fn collapsed_groups_retain_members_in_both_presentations() {
    let mut tab = fixture();
    let archives = GroupKey::Family(FileFamily::Archives);
    tab.collapsed_groups.insert(archives.clone());
    for grid in [false, true] {
        let presentation = tab.presentation(grid, if grid { 3 } else { 1 });
        let indices = &presentation.groups[&archives];
        assert_eq!(indices.len(), 2);
        assert!(presentation
            .rows
            .iter()
            .any(|row| matches!(row, DisplayRow::Header(group, 2) if group == &archives)));
        assert!(!presentation.rows.iter().any(|row| matches!(row, DisplayRow::Files(visible) if visible.iter().any(|index| indices.contains(index)))));
    }
    toggle_group(&mut tab, &archives);
    assert_eq!(state(&tab, &archives), GroupSelection::All);
    assert!(tab.collapsed_groups.contains(&archives));
}

#[test]
fn filtered_out_files_are_not_selected_or_deselected_with_the_group() {
    let mut tab = fixture();
    let archives = GroupKey::Family(FileFamily::Archives);
    tab.filter_name = "a.zip".into();
    tab.update_cache();
    tab.selected_paths.insert("C:/b.7z".into());
    toggle_group(&mut tab, &archives);
    assert!(tab.selected_paths.contains("C:/a.zip"));
    toggle_group(&mut tab, &archives);
    assert_eq!(tab.selected_paths, HashSet::from(["C:/b.7z".into()]));
}

#[test]
fn selection_changes_reuse_member_snapshot_and_no_grouping_is_a_noop() {
    let mut tab = fixture();
    let archives = GroupKey::Family(FileFamily::Archives);
    let before = tab.presentation(false, 1);
    toggle_group(&mut tab, &archives);
    let after = tab.presentation(false, 1);
    assert!(Arc::ptr_eq(&before, &after));
    tab.group_mode = GroupMode::None;
    toggle_group(&mut tab, &archives);
    assert_eq!(tab.selected_paths.len(), 2);
}
