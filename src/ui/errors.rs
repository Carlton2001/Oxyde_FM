use crate::ui::i18n::Message;
use oxyde_lib::models::CommandError;

pub fn redo_failure(error: &CommandError) -> (Message, String) {
    if let CommandError::HistoryCopy(failure) = error {
        let message = match failure.issue {
            oxyde_lib::models::error::HistoryCopyIssue::SourceChanged => {
                Message::RedoCopySourceChanged
            }
            oxyde_lib::models::error::HistoryCopyIssue::Unverified => Message::RedoCopyUnverified,
        };
        (message, failure.path.clone())
    } else {
        (Message::RedoFailed, format!("{error:?}"))
    }
}

pub fn path_load_status(error: &CommandError) -> crate::ui::workspace::TabLoadStatus {
    if let Some(failure) =
        oxyde_lib::models::error::ArchiveLimitFailure::from_message(&error.to_string())
    {
        crate::ui::workspace::TabLoadStatus::ArchiveLimit(failure)
    } else {
        crate::ui::workspace::TabLoadStatus::Error(path_load_message(error))
    }
}

/// Maps backend failures to concise UI message keys.
/// Technical details remain available in logs instead of leaking OS error strings to users.
pub fn path_load_message(error: &CommandError) -> Message {
    match error {
        CommandError::PathNotFound(_) => Message::PathNotFound,
        CommandError::AccessDenied(_) => Message::PathAccessDenied,
        _ => Message::PathLoadFailed,
    }
}

/// Store a message key, so an error is translated in the current UI language.
pub fn rename_message(error: &CommandError) -> Message {
    if oxyde_lib::models::error::ArchiveSourceConflict::from_message(&error.to_string()) {
        return Message::ArchiveSourceChanged;
    }
    match error {
        CommandError::IoError(message)
            if message.to_ascii_lowercase().contains("already exists") =>
        {
            Message::RenameAlreadyExists
        }
        CommandError::PathNotFound(_) => Message::RenameNotFound,
        CommandError::AccessDenied(_) => Message::RenameAccessDenied,
        CommandError::IoError(_) | CommandError::PathError(_) => Message::RenameInvalidName,
        _ => Message::RenameUnavailable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn redo_copy_failures_explain_the_cause_with_a_literal_path() {
        use crate::ui::i18n::I18n;
        use fluent_bundle::FluentArgs;
        use oxyde_lib::models::error::{HistoryCopyFailure, HistoryCopyIssue};
        use oxyde_lib::models::preferences::Language;
        let path = "C:\\école\\{ $detail }.txt";
        for (issue, expected) in [
            (
                HistoryCopyIssue::SourceChanged,
                Message::RedoCopySourceChanged,
            ),
            (HistoryCopyIssue::Unverified, Message::RedoCopyUnverified),
        ] {
            let error = CommandError::from(std::io::Error::other(HistoryCopyFailure {
                issue,
                path: path.into(),
            }));
            let (message, detail) = redo_failure(&error);
            assert_eq!(message, expected);
            assert_eq!(detail, path);
            for language in Language::ALL {
                let mut args = FluentArgs::new();
                args.set("detail", detail.as_str());
                let rendered = I18n::for_language(language).text_with_args(message, &args);
                assert!(rendered.contains(path), "{language:?}: {rendered}");
                assert!(!rendered.contains("Redo copy source changed"));
                assert!(!rendered.contains(message.key()));
            }
        }
        assert_eq!(
            redo_failure(&CommandError::Other("unrelated error".into())).0,
            Message::RedoFailed
        );
    }
    #[test]
    fn archive_rename_conflicts_explain_that_the_current_archive_was_preserved() {
        assert_eq!(
            rename_message(&CommandError::IoError(
                oxyde_lib::models::error::ArchiveSourceConflict.to_string()
            )),
            Message::ArchiveSourceChanged
        );
    }

    #[test]
    fn archive_listing_limits_retain_the_threshold_in_the_tab() {
        use oxyde_lib::models::error::{ArchiveLimitFailure, ArchiveLimitKind};
        let failure = ArchiveLimitFailure {
            kind: ArchiveLimitKind::Entries,
            limit: 100_000,
        };
        let error = CommandError::ArchiveError(format!("Decoder failed: {failure}"));
        assert!(
            matches!(path_load_status(&error), crate::ui::workspace::TabLoadStatus::ArchiveLimit(actual) if actual == failure)
        );
        assert!(matches!(
            path_load_status(&CommandError::AccessDenied("private".into())),
            crate::ui::workspace::TabLoadStatus::Error(Message::PathAccessDenied)
        ));
    }

    #[test]
    fn path_errors_map_to_translation_keys_without_operating_system_details() {
        assert_eq!(
            path_load_message(&CommandError::PathNotFound("E:\\missing".into())),
            Message::PathNotFound
        );
        assert_eq!(
            path_load_message(&CommandError::AccessDenied("C:\\private".into())),
            Message::PathAccessDenied
        );
        assert_eq!(
            path_load_message(&CommandError::IoError("os error 3".into())),
            Message::PathLoadFailed
        );
    }

    #[test]
    fn rename_conflicts_are_localized_without_os_error_details() {
        assert_eq!(
            rename_message(&CommandError::IoError(
                "Rename target already exists: D:\\Tests\\2".to_string()
            )),
            Message::RenameAlreadyExists
        );
    }
}
