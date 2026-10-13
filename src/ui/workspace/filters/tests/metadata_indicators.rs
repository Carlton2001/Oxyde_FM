use crate::ui::workspace::{SizeCategory, TabState};
use oxyde_lib::models::FileEntry;

#[test]
fn all_or_no_present_values_keep_metadata_indicators_inactive() {
    let mut tab = TabState::new("filters".into(), "C:\\".into());
    tab.files = vec![
        FileEntry { name: "small.txt".into(), size: 100, ..Default::default() },
        FileEntry { name: "large.mov".into(), size: 2_000_000_000, ..Default::default() },
    ];
    assert_eq!(tab.metadata_filter_flags(), (false, false, false));
    tab.filter_types.extend(["TXT".into(), "MOV".into()]);
    tab.filter_sizes.extend([SizeCategory::Minuscule, SizeCategory::Huge]);
    assert_eq!(tab.metadata_filter_flags(), (false, false, false));
    tab.filter_types.remove("MOV");
    tab.filter_sizes.remove(&SizeCategory::Huge);
    assert_eq!(tab.metadata_filter_flags(), (true, true, false));
    tab.filter_types.clear();
    tab.filter_sizes.clear();
    assert_eq!(tab.metadata_filter_flags(), (false, false, false));
}

#[test]
fn empty_tab_has_no_metadata_filter_indicators() {
    let mut tab = TabState::new("empty".into(), "C:\\".into());
    tab.filter_types.insert("TXT".into());
    tab.filter_sizes.insert(SizeCategory::Huge);
    assert_eq!(tab.metadata_filter_flags(), (false, false, false));
}
