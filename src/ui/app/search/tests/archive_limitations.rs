use super::*;
use oxyde_lib::{commands::search::ArchiveSearchLimitation, models::preferences::Language};

#[::core::prelude::v1::test]
fn unsupported_criteria_are_translated_in_all_languages_and_keep_the_archive_path() {
    for (reason, message) in [
        (
            ArchiveSearchLimitation::ContentNotSearched,
            Message::QuickSearchArchiveContentNotSearched,
        ),
        (
            ArchiveSearchLimitation::DatesUnavailable,
            Message::QuickSearchArchiveDatesUnavailable,
        ),
    ] {
        let mut warnings = ArchiveSearchWarnings::default();
        let path = r"C:\Données\日本語 { $count }.zip";
        warnings.record(path.into(), format!("Archive Error: {}", reason.message()));
        for language in Language::ALL {
            let i18n = crate::ui::i18n::I18n::for_language(language);
            let detail = warnings.detail(&i18n);
            assert!(detail.contains(path));
            assert!(detail.contains(i18n.text(message).as_ref()));
            assert!(!detail.contains(reason.message()));
            assert!(!detail.contains(i18n.text(Message::QuickSearchArchiveUnreadable).as_ref()));
            assert!(warnings.label(&i18n).contains('1'));
        }
    }
}
