use super::*;
use crate::ui::workspace::FileUpdate;

fn trash_tab() -> TabState {
    let mut tab = TabState::new("trash".into(), "trash://".into());
    tab.files = vec![
        FileEntry { name: "Résumé annuel.txt".into(), path: "$R123456.txt".into(),
            original_path: Some("Documents/Résumé annuel.txt".into()), ..Default::default() },
        FileEntry { name: "Photo.jpg".into(), path: "$R654321.jpg".into(),
            original_path: Some("Pictures/Photo.jpg".into()), ..Default::default() },
    ].into();
    tab
}

#[test]
fn trash_search_matches_display_names_and_preserves_item_identities() {
    let mut tab = trash_tab();
    for query in ["RÉSUMÉ", "*ANNUEL.?xt", " annuel "] {
        tab.search_text = query.into();
        let prepared = tab.preparation(FileUpdate::Current).calculate();
        assert!(tab.apply_preparation(prepared));
        assert_eq!(tab.filtered_files().len(), 1);
        assert_eq!(tab.filtered_files()[0].path, "$R123456.txt");
        assert_eq!(tab.files.len(), 2);
        assert_eq!(tab.path, "trash://");
    }
    tab.search_text = "$R123456".into();
    tab.update_cache();
    assert!(tab.filtered_files().is_empty());
    tab.search_text.clear();
    tab.update_cache();
    assert_eq!(tab.filtered_files().len(), 2);
}

#[test]
fn trash_search_combines_with_column_filters_and_new_listing_batches() {
    let mut tab = trash_tab();
    tab.search_text = "résumé".into();
    tab.filter_name = "photo".into();
    tab.update_cache();
    assert!(tab.filtered_files().is_empty());
    tab.filter_name.clear();
    let additions = vec![FileEntry { name: "Résumé mensuel.txt".into(),
        path: "$R987654.txt".into(), ..Default::default() }];
    let prepared = tab.preparation(FileUpdate::Append(additions.into())).calculate();
    assert!(tab.apply_preparation(prepared));
    assert_eq!(tab.filtered_files().len(), 2);
    tab.path = "Documents".into();
    tab.update_cache();
    assert_eq!(tab.filtered_files().len(), 3);
}

#[test]
fn trash_search_banner_query_follows_typing_and_clearing() {
    let mut tab = trash_tab();
    assert_eq!(tab.quick_search_query(), None);
    tab.search_text = " résumé ".into();
    assert_eq!(tab.quick_search_query(), Some("résumé"));
    // Trash filtering has no filesystem search worker state.
    assert!(tab.quick_search.is_none());
    tab.search_text.clear();
    assert_eq!(tab.quick_search_query(), None);
    tab.search_text = "   ".into();
    assert_eq!(tab.quick_search_query(), None);
    tab.search_text = "résumé".into();
    tab.path = "Documents".into();
    assert_eq!(tab.quick_search_query(), None);
}
