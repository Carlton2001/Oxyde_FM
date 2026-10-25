use super::*;
use crate::ui::i18n::Language;

#[test]
fn incomplete_office_search_is_explicit_and_localized_without_exposing_diagnostics() {
    let mut warnings = ContentSearchWarnings::default();
    let path = r"C:\école\日本語 { $count }.docx";
    warnings.record(path.into());
    warnings.record("other.xlsx".into());
    for language in Language::ALL {
        let i18n = I18n::for_language(language);
        assert!(warnings.label(&i18n).contains('2'));
        let detail = warnings.detail(&i18n);
        assert!(detail.contains(path));
        assert!(!detail.contains("other.xlsx"));
        assert!(!detail.contains("[quick-search"));
        assert!(!warnings.label(&i18n).contains("[quick-search"));
    }
}
