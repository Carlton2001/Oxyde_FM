//! Display conventions for the supported languages. File data remains numeric.
use chrono::{DateTime, Local, NaiveDateTime};
pub use oxyde_lib::models::preferences::DateFormat;
use oxyde_lib::models::preferences::Language;
pub struct DateFormatPreference(pub DateFormat);
impl gpui::Global for DateFormatPreference {}

pub fn drive_label(label: &str, path: &str, i18n: &crate::ui::i18n::I18n) -> String {
    let path = path.trim_end_matches(['/', '\\']);
    if label.is_empty() || label == "Local Disk" {
        let mut args = crate::ui::i18n::FluentArgs::new();
        args.set("drive", path);
        i18n.text_with_args(crate::ui::i18n::Message::LocalDrive, &args)
            .to_string()
    } else {
        format!("{label} ({path})")
    }
}
pub fn date_format(cx: &gpui::App) -> DateFormat {
    cx.global::<DateFormatPreference>().0
}

pub fn format_number(number: u64, language: Language) -> String {
    let digits = number.to_string();
    let separator = match language {
        Language::French => '\u{202f}',
        Language::English => ',',
        Language::German | Language::Spanish | Language::Italian => '.',
    };
    let mut result = String::new();
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            result.push(separator);
        }
        result.push(ch);
    }
    result
}

pub fn format_size(bytes: u64, is_dir: bool, is_calculated: bool, language: Language) -> String {
    if is_dir && !is_calculated {
        return String::new();
    }
    let units = match language {
        Language::French => ["o", "Ko", "Mo", "Go", "To", "Po", "Eo"],
        _ => ["B", "KB", "MB", "GB", "TB", "PB", "EB"],
    };
    let mut unit = 0;
    let mut divisor = 1u128;
    while unit < units.len() - 1 && u128::from(bytes) >= divisor * 1024 {
        unit += 1;
        divisor *= 1024;
    }
    if unit == 0 {
        return format!("{} {}", format_number(bytes, language), units[0]);
    }
    let mut tenths = (u128::from(bytes) * 10 + divisor / 2) / divisor;
    if tenths >= 10240 && unit < units.len() - 1 {
        unit += 1;
        divisor *= 1024;
        tenths = (u128::from(bytes) * 10 + divisor / 2) / divisor;
    }
    let decimal = match language {
        Language::French | Language::German | Language::Spanish | Language::Italian => ',',
        Language::English => '.',
    };
    format!(
        "{}{decimal}{} {}",
        format_number((tenths / 10) as u64, language),
        tenths % 10,
        units[unit]
    )
}

pub fn format_date(timestamp_ms: u64, format: DateFormat) -> String {
    if timestamp_ms == 0 {
        return "–".into();
    }
    i64::try_from(timestamp_ms)
        .ok()
        .and_then(DateTime::from_timestamp_millis)
        .map(|date| format_datetime(date.with_timezone(&Local).naive_local(), format))
        .unwrap_or_else(|| "–".into())
}

fn format_datetime(date: NaiveDateTime, format: DateFormat) -> String {
    let pattern = match format {
        DateFormat::European => "%d/%m/%Y %H:%M:%S",
        DateFormat::Us => "%m/%d/%Y %H:%M:%S",
        DateFormat::Iso => "%Y-%m-%d %H:%M:%S",
    };
    date.format(pattern).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn trash_restore_failures_are_clear_in_every_language() {
        use crate::ui::i18n::{I18n, Message};
        use oxyde_lib::models::error::{TrashRestoreFailure, TrashRestoreIssue};
        for language in [Language::French, Language::English, Language::German, Language::Spanish, Language::Italian] {
            let i18n = I18n::for_language(language);
            for (issue, key) in [
                (TrashRestoreIssue::Ambiguous, Message::TrashRestoreAmbiguous),
                (TrashRestoreIssue::Changed, Message::TrashRestoreChanged),
                (TrashRestoreIssue::Conflict, Message::TrashRestoreConflict),
                (TrashRestoreIssue::Unavailable, Message::TrashRestoreUnavailable),
            ] {
                let error = TrashRestoreFailure { issue, path: "C:/école/空".into(), detail: "Native operation failed".into() };
                for detail in [error.to_string(), format!("TrashError({:?})", error.to_string())] {
                    let displayed = error_detail(&detail, &i18n);
                    assert_eq!(displayed, i18n.text(key).to_string());
                    assert!(!displayed.contains("Trash restore ["));
                    assert!(!displayed.starts_with('['));
                    assert!(displayed.len() > 100);
                }
            }
            assert_eq!(error_detail("Unrelated trash error", &i18n), "Unrelated trash error");
        }
    }
    #[test]
    fn selected_source_failures_explain_partial_results_in_all_languages() {
        use crate::ui::i18n::{I18n, Message};
        for language in [Language::French, Language::English, Language::German, Language::Spanish, Language::Italian] {
            let i18n = I18n::for_language(language);
            for (changed, key) in [(true, Message::TransferSourceChanged), (false, Message::TransferSourceUnverified)] {
                let failure = oxyde_lib::models::error::SourceSelectionFailure { changed, path: "C:/écriture/空".into() };
                let detail = format!("Cannot copy source: {failure}");
                let displayed = error_detail(&detail, &i18n);
                assert_eq!(displayed, i18n.text(key).to_string());
                assert!(!displayed.contains("Source selection ["));
                assert!(!displayed.starts_with('['));
                assert!(displayed.len() > 100);
            }
            let unrelated = "File missing: source selection.txt";
            assert_eq!(error_detail(unrelated, &i18n), unrelated);
        }
    }

    #[test]
    fn archive_source_conflicts_are_translated_in_all_languages() {
        for (language, expected) in [
            (Language::French, "L’archive a changé"),
            (Language::English, "The archive changed"),
            (Language::German, "Das Archiv wurde"),
            (Language::Spanish, "El archivo comprimido cambió"),
            (Language::Italian, "L’archivio è cambiato"),
        ] {
            let i18n = crate::ui::i18n::I18n::for_language(language);
            let detail = format!(
                "Archive Error: {}",
                oxyde_lib::models::error::ArchiveSourceConflict
            );
            let displayed = error_detail(&detail, &i18n);
            assert!(displayed.starts_with(expected), "{displayed}");
            assert!(!displayed.contains(oxyde_lib::models::error::ArchiveSourceConflict::MARKER));
            let unrelated = format!(
                "File missing: report {}.zip",
                oxyde_lib::models::error::ArchiveSourceConflict::MARKER
            );
            assert_eq!(error_detail(&unrelated, &i18n), unrelated);
        }
    }
    #[test]
    fn archive_failures_display_localized_thresholds_in_all_languages() {
        use oxyde_lib::models::error::{ArchiveLimitFailure, ArchiveLimitKind};
        for language in [
            Language::French,
            Language::English,
            Language::German,
            Language::Spanish,
            Language::Italian,
        ] {
            let i18n = crate::ui::i18n::I18n::for_language(language);
            for (kind, limit, expected) in [
                (
                    ArchiveLimitKind::Bytes,
                    256 * 1024 * 1024,
                    format_size(256 * 1024 * 1024, false, true, language),
                ),
                (
                    ArchiveLimitKind::Entries,
                    100_000,
                    format_number(100_000, language),
                ),
                (ArchiveLimitKind::Depth, 64, "64".into()),
                (ArchiveLimitKind::Seconds, 120, "2".into()),
                (
                    ArchiveLimitKind::Decoded,
                    8 * 1024 * 1024 * 1024,
                    format_size(8 * 1024 * 1024 * 1024, false, true, language),
                ),
            ] {
                let wrapped = format!("Archive Error: {}", ArchiveLimitFailure { kind, limit });
                let displayed = error_detail(&wrapped, &i18n);
                assert!(displayed.contains(&expected), "{displayed}");
                assert!(
                    !displayed.contains("Archive Error")
                        && !displayed.contains("Archive limit exceeded"),
                    "{displayed}"
                );
                assert!(
                    !displayed.contains("[archive-limit-") && !displayed.contains("{$"),
                    "{displayed}"
                );
            }
            assert_eq!(
                error_detail("Unrelated IO error", &i18n),
                "Unrelated IO error"
            );
        }
    }
    #[test]
    fn numbers_sizes_and_boundaries_follow_language() {
        assert_eq!(
            format_number(u64::MAX, Language::English),
            "18,446,744,073,709,551,615"
        );
        assert_eq!(
            format_number(1234567, Language::French),
            "1\u{202f}234\u{202f}567"
        );
        assert_eq!(format_size(1536, false, false, Language::French), "1,5 Ko");
        assert_eq!(format_size(1536, false, false, Language::English), "1.5 KB");
        for language in [Language::German, Language::Spanish, Language::Italian] {
            assert_eq!(format_number(1234567, language), "1.234.567");
            assert_eq!(format_size(1536, false, false, language), "1,5 KB");
            assert_eq!(format_size(1023, false, false, language), "1.023 B");
            assert_eq!(format_size(1048575, false, false, language), "1,0 MB");
        }
        assert_eq!(format_size(0, false, false, Language::English), "0 B");
        assert_eq!(
            format_size(1023, false, false, Language::French),
            "1\u{202f}023 o"
        );
        assert_eq!(
            format_size(1048575, false, false, Language::English),
            "1.0 MB"
        );
        assert_eq!(
            format_size(u64::MAX, false, false, Language::English),
            "16.0 EB"
        );
        assert_eq!(format_size(99, true, false, Language::French), "");
        assert_eq!(format_size(0, true, true, Language::French), "0 o");
    }
    #[test]
    fn date_order_is_localized_without_changing_the_clock() {
        let date = chrono::NaiveDate::from_ymd_opt(2026, 9, 28)
            .unwrap()
            .and_hms_opt(13, 5, 7)
            .unwrap();
        assert_eq!(
            format_datetime(date, DateFormat::European),
            "28/09/2026 13:05:07"
        );
        assert_eq!(format_datetime(date, DateFormat::Us), "09/28/2026 13:05:07");
        assert_eq!(
            format_datetime(date, DateFormat::Iso),
            "2026-09-28 13:05:07"
        );
        assert_eq!(format_date(0, DateFormat::European), "–");
        assert_eq!(format_date(u64::MAX, DateFormat::Us), "–");
    }

    #[test]
    fn drive_labels_translate_system_defaults_and_preserve_volume_names() {
        for (language, expected) in [
            (Language::French, "Disque local"),
            (Language::English, "Local Disk"),
            (Language::German, "Lokaler Datenträger"),
            (Language::Spanish, "Disco local"),
            (Language::Italian, "Disco locale"),
        ] {
            let i18n = crate::ui::i18n::I18n::for_language(language);
            for label in ["", "Local Disk"] {
                let displayed = drive_label(label, "C:\\", &i18n);
                assert!(displayed.starts_with(expected));
                assert!(displayed.contains("C:"));
                assert!(!displayed.contains('\\'));
                assert!(!displayed.contains("[local-drive]"));
            }
            assert_eq!(
                drive_label("Mes données", "D:\\", &i18n),
                "Mes données (D:)"
            );
        }
    }
}

/// Resolve archive limits at render time, including stored operation failures.
pub fn error_detail(detail: &str, i18n: &crate::ui::i18n::I18n) -> String {
    use crate::ui::i18n::{FluentArgs, Message};
    use oxyde_lib::models::error::{ArchiveLimitFailure, ArchiveLimitKind};
    if let Some(issue) = oxyde_lib::models::error::TrashRestoreFailure::from_message(detail) {
        use oxyde_lib::models::error::TrashRestoreIssue;
        let message = match issue {
            TrashRestoreIssue::Ambiguous => Message::TrashRestoreAmbiguous,
            TrashRestoreIssue::Changed => Message::TrashRestoreChanged,
            TrashRestoreIssue::Conflict => Message::TrashRestoreConflict,
            TrashRestoreIssue::Unavailable => Message::TrashRestoreUnavailable,
        };
        return i18n.text(message).to_string();
    }
    if let Some(changed) = oxyde_lib::models::error::SourceSelectionFailure::from_message(detail) {
        return i18n.text(if changed { Message::TransferSourceChanged } else { Message::TransferSourceUnverified }).to_string();
    }
    if oxyde_lib::models::error::ArchiveSourceConflict::from_message(detail) {
        return i18n.text(Message::ArchiveSourceChanged).to_string();
    }
    let Some(failure) = ArchiveLimitFailure::from_message(detail) else {
        return detail.into();
    };
    let message = match failure.kind {
        ArchiveLimitKind::Bytes => Message::ArchiveLimitBytes,
        ArchiveLimitKind::Entries => Message::ArchiveLimitEntries,
        ArchiveLimitKind::Depth => Message::ArchiveLimitDepth,
        ArchiveLimitKind::Seconds => Message::ArchiveLimitTime,
        ArchiveLimitKind::Decoded => Message::ArchiveLimitDecoded,
    };
    let limit = match failure.kind {
        ArchiveLimitKind::Bytes | ArchiveLimitKind::Decoded => {
            format_size(failure.limit, false, true, i18n.language())
        }
        ArchiveLimitKind::Seconds => format_number(failure.limit.div_ceil(60), i18n.language()),
        _ => format_number(failure.limit, i18n.language()),
    };
    let mut args = FluentArgs::new();
    args.set("limit", limit);
    i18n.text_with_args(message, &args).to_string()
}
