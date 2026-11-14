use crate::ui::date_groups::DateGroup;
use crate::ui::workspace::{DateRange, FilterValue, SizeCategory, SizeRange, TabState};
use oxyde_lib::models::FileEntry;

fn tab(files: Vec<FileEntry>) -> TabState {
    let mut tab = TabState::new("criteria".into(), "C:\\".into());
    tab.files = std::sync::Arc::new(files);
    tab
}

fn kept(tab: &mut TabState) -> Vec<String> {
    tab.update_cache();
    tab.filtered_files().iter().map(|file| file.name.clone()).collect()
}

#[test]
fn type_filter_keeps_the_kinds_its_list_offers() {
    let mut tab = tab(vec![
        FileEntry { name: "README".into(), ..Default::default() },
        FileEntry { name: "notes.txt".into(), ..Default::default() },
        FileEntry { name: "shortcut.txt".into(), is_symlink: true, ..Default::default() },
        FileEntry { name: "docs".into(), is_dir: true, ..Default::default() },
    ]);
    tab.filter_types.insert("Fichier".into());
    assert_eq!(kept(&mut tab), ["README"]);
    tab.filter_types = ["Symlink".into()].into();
    assert_eq!(kept(&mut tab), ["shortcut.txt"]);
    tab.filter_types = ["TXT".into(), "Dossier".into()].into();
    assert_eq!(kept(&mut tab), ["notes.txt", "docs"]);
}

#[test]
fn name_filter_reads_text_patterns_and_expressions() {
    let mut tab = tab(["Report.PDF", "report-old.pdf", "notes.txt"].map(|name| FileEntry { name: name.into(), ..Default::default() }).to_vec());
    tab.filter_name = "report".into();
    assert_eq!(kept(&mut tab), ["Report.PDF", "report-old.pdf"]);
    tab.filter_name = "*.pdf".into();
    assert_eq!(kept(&mut tab), ["Report.PDF", "report-old.pdf"]);
    tab.filter_name = "r*t.pdf".into();
    assert_eq!(kept(&mut tab), ["Report.PDF"]);
    tab.filter_name_regex = true;
    tab.filter_name = r"^\w+\.(txt|pdf)$".into();
    assert_eq!(kept(&mut tab), ["Report.PDF", "notes.txt"]);
    // An expression being typed filters nothing rather than everything.
    tab.filter_name = "(".into();
    assert_eq!(kept(&mut tab).len(), 3);
    assert!(!crate::ui::workspace::NameMatcher::is_valid("(", true));
    assert!(crate::ui::workspace::NameMatcher::is_valid("(", false));
}

#[test]
fn ranges_include_their_bounds_and_replace_checked_values() {
    use chrono::TimeZone;
    let day = |d: u32| chrono::Local.with_ymd_and_hms(2026, 3, d, 12, 0, 0).single().unwrap().timestamp_millis() as u64;
    let mut tab = tab(vec![
        FileEntry { name: "a".into(), size: 1024, modified: day(1), ..Default::default() },
        FileEntry { name: "b".into(), size: 2048, modified: day(2), ..Default::default() },
        FileEntry { name: "c".into(), size: 4096, modified: day(3), ..Default::default() },
    ]);
    tab.set_value_checked(FilterValue::Size(SizeCategory::Huge), true);
    tab.set_size_range(SizeRange { min: Some(2048), max: None });
    assert!(tab.filter_sizes.is_empty());
    assert_eq!(kept(&mut tab), ["b", "c"]);
    tab.set_size_range(SizeRange { min: None, max: Some(2048) });
    assert_eq!(kept(&mut tab), ["a", "b"]);
    tab.set_size_range(SizeRange::default());
    assert_eq!(tab.filter_size_range, None);

    let date = |d: u32| chrono::NaiveDate::from_ymd_opt(2026, 3, d);
    tab.set_date_range(DateRange { from: date(2), to: date(2) });
    assert_eq!(kept(&mut tab), ["b"]);
    tab.set_value_checked(FilterValue::Date(DateGroup::Older), true);
    assert_eq!(tab.filter_date_range, None);
}

#[test]
fn clearing_a_column_keeps_the_others() {
    let mut tab = tab(vec![FileEntry { name: "a.txt".into(), ..Default::default() }]);
    tab.filter_name = "a".into();
    tab.filter_types.insert("TXT".into());
    tab.set_date_range(DateRange { from: chrono::NaiveDate::from_ymd_opt(2026, 1, 1), to: None });
    assert!(tab.has_column_filters());
    tab.clear_column_filter(crate::ui::workspace::FilterColumn::Date);
    assert_eq!(tab.filter_date_range, None);
    assert_eq!(tab.filter_name, "a");
    tab.clear_column_filters();
    assert!(!tab.has_column_filters());
}
