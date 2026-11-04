use crate::ui::workspace::{SizeCategory, TabState};
use oxyde_lib::models::FileEntry;

#[test]
fn all_or_no_present_values_keep_metadata_indicators_inactive() {
    let mut tab = TabState::new("filters".into(), "C:\\".into());
    tab.files = std::sync::Arc::new(vec![
        FileEntry { name: "small.txt".into(), size: 100, ..Default::default() },
        FileEntry { name: "large.mov".into(), size: 2_000_000_000, ..Default::default() },
    ]);
    assert_eq!(tab.column_filter_flags(), (false, false, false, false));
    tab.filter_types.extend(["TXT".into(), "MOV".into()]);
    tab.filter_sizes.extend([SizeCategory::Minuscule, SizeCategory::Huge]);
    assert_eq!(tab.column_filter_flags(), (false, false, false, false));
    tab.filter_types.remove("MOV");
    tab.filter_sizes.remove(&SizeCategory::Huge);
    assert_eq!(tab.column_filter_flags(), (true, true, false, false));
    tab.filter_types.clear();
    tab.filter_sizes.clear();
    assert_eq!(tab.column_filter_flags(), (false, false, false, false));
}

#[test]
fn empty_tab_has_no_metadata_filter_indicators() {
    let mut tab = TabState::new("empty".into(), "C:\\".into());
    tab.filter_types.insert("TXT".into());
    tab.filter_sizes.insert(SizeCategory::Huge);
    assert_eq!(tab.column_filter_flags(), (false, false, false, false));
}

#[test]
fn permission_filter_keeps_selected_modes_and_flags_only_a_partial_choice() {
    let mut tab = TabState::new("permissions".into(), "/home".into());
    tab.files = std::sync::Arc::new(vec![
        FileEntry { name: "script.sh".into(), mode: Some(0o755), ..Default::default() },
        FileEntry { name: "notes.txt".into(), mode: Some(0o644), ..Default::default() },
        FileEntry { name: "virtual".into(), mode: None, ..Default::default() },
    ]);
    tab.filter_permissions.extend([0o755, 0o644]);
    assert_eq!(tab.column_filter_flags(), (false, false, false, false));
    tab.filter_permissions.remove(&0o644);
    assert_eq!(tab.column_filter_flags(), (false, false, false, true));
    tab.update_cache();
    let kept: Vec<_> = tab.filtered_files().iter().map(|file| file.name.as_str()).collect();
    assert_eq!(kept, ["script.sh"]);
}
