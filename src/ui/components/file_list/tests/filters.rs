use super::{filter_entries, filter_summary};
use crate::ui::i18n::{I18n, Language};
use crate::ui::workspace::{FilterColumn, FilterValue, SizeCategory, SizeRange, TabState};
use oxyde_lib::models::FileEntry;

fn entry(name: &str, size: u64) -> FileEntry {
    FileEntry { name: name.into(), path: format!("C:\\{name}"), size, ..Default::default() }
}

fn tab(files: Vec<FileEntry>) -> TabState {
    let mut tab = TabState::new("filters".into(), "C:\\".into());
    tab.files = std::sync::Arc::new(files);
    tab
}

#[test]
fn type_values_list_kinds_first_then_extensions() {
    let tab = tab(vec![
        entry("b.txt", 1),
        FileEntry { is_dir: true, ..entry("docs", 0) },
        entry("README", 1),
        entry("a.PDF", 1),
        FileEntry { is_symlink: true, ..entry("link", 0) },
    ]);
    let i18n = I18n::for_language(Language::English);
    let values: Vec<_> = filter_entries(&tab, FilterColumn::Type, &i18n).into_iter().map(|entry| entry.value).collect();
    assert_eq!(values, ["Dossier", "Fichier", "Symlink", "PDF", "TXT"].map(|label| FilterValue::Type(label.into())));
}

#[test]
fn counts_follow_the_other_columns_and_keep_values_checked_elsewhere() {
    let mut tab = tab(vec![entry("small.txt", 100), entry("large.txt", 2_000_000_000), entry("large.mov", 2_000_000_000)]);
    tab.filter_sizes.insert(SizeCategory::Huge);
    tab.filter_types.insert("ZIP".into());
    let i18n = I18n::for_language(Language::English);
    let types = filter_entries(&tab, FilterColumn::Type, &i18n);
    let count = |label: &str| types.iter().find(|entry| entry.value == FilterValue::Type(label.into())).map(|entry| entry.count);
    // The size filter keeps the large files only; the type filter does not count against itself.
    assert_eq!(count("TXT"), Some(1));
    assert_eq!(count("MOV"), Some(1));
    // Checked in another folder, absent here: still listed, to be unchecked.
    assert_eq!(count("ZIP"), Some(0));
    let sizes = filter_entries(&tab, FilterColumn::Size, &i18n);
    // ZIP keeps nothing: every size counts zero.
    assert!(sizes.iter().all(|entry| entry.count == 0));
}

#[test]
fn summaries_name_values_and_bounds() {
    let mut tab = tab(vec![entry("a.txt", 1)]);
    let i18n = I18n::for_language(Language::English);
    let format = crate::ui::format::DateFormat::Iso;
    assert_eq!(filter_summary(&tab, FilterColumn::Type, &i18n, format), None);
    tab.filter_types.extend(["TXT".into(), "Dossier".into(), "PDF".into(), "ZIP".into()]);
    assert_eq!(filter_summary(&tab, FilterColumn::Type, &i18n, format).as_deref(), Some("Folder, PDF, TXT (+1)"));
    tab.filter_size_range = Some(SizeRange { min: Some(1024 * 1024), max: None });
    assert_eq!(filter_summary(&tab, FilterColumn::Size, &i18n, format).as_deref(), Some("≥ 1.0 MB"));
}
