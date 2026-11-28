use crate::ui::date_groups::DateGroup;
use crate::ui::workspace::{FilterColumn, SizeCategory, TabState};
use oxyde_lib::models::FileEntry;

#[test]
fn checking_values_of_a_column_keeps_its_facet_key_and_other_changes_do_not() {
    let today = chrono::NaiveDate::from_ymd_opt(2026, 11, 28).unwrap();
    let mut tab = TabState::new("facets".into(), "C:\\".into());
    tab.files = std::sync::Arc::new(vec![FileEntry { name: "a.txt".into(), ..Default::default() }]);
    let key = tab.facet_key(FilterColumn::Type, today);
    tab.filter_types.insert("TXT".into());
    assert!(tab.facet_key(FilterColumn::Type, today) == key);
    tab.filter_sizes.insert(SizeCategory::Minuscule);
    let sized = tab.facet_key(FilterColumn::Type, today);
    assert!(sized != key);
    tab.filter_sizes.clear();
    tab.filter_dates.insert(DateGroup::ALL[0]);
    assert!(tab.facet_key(FilterColumn::Date, today) != tab.facet_key(FilterColumn::Type, today));
    tab.filter_dates.clear();
    assert!(tab.facet_key(FilterColumn::Type, today) == key);
    assert!(tab.facet_key(FilterColumn::Type, today.succ_opt().unwrap()) != key);
    tab.files = std::sync::Arc::new(tab.files.to_vec());
    assert!(tab.facet_key(FilterColumn::Type, today) != key);
}
