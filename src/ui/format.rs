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
        Language::English => ["B", "KB", "MB", "GB", "TB", "PB", "EB"],
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
        Language::French => ',',
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
