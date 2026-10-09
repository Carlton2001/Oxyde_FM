use crate::ui::i18n::Message;
use oxyde_lib::models::CommandError;

pub fn undo_failure(error: &CommandError) -> (Message, String) {
    if let CommandError::HistoryRename(failure) = error {
        let message = match failure.issue {
            oxyde_lib::models::error::HistoryIdentityIssue::Changed => Message::UndoRenameChanged,
            oxyde_lib::models::error::HistoryIdentityIssue::Unverified => {
                Message::UndoRenameUnverified
            }
        };
        return (message, failure.path.clone());
    }
    if let CommandError::HistoryMove(failure) = error {
        use oxyde_lib::models::error::HistoryMoveIssue as Issue;
        let message = match failure.issue {
            Issue::Changed => Message::UndoMoveChanged,
            Issue::Unverified => Message::UndoMoveUnverified,
            Issue::Conflict | Issue::Incomplete => Message::UndoFailed,
        };
        return (message, failure.detail.clone());
    }
    if let CommandError::HistoryUndoRecovery(failure) = error {
        return (Message::UndoRecoveryIncomplete, failure.locations.clone());
    }

    if let CommandError::NewFolderHistory(failure) = error {
        use oxyde_lib::models::error::NewFolderHistoryIssue as Issue;
        let message = match failure.issue {
            Issue::Changed | Issue::Conflict => Message::UndoNewFolderChanged,
            Issue::Unverified => Message::UndoNewFolderUnverified,
            Issue::NotEmpty => Message::UndoNewFolderNotEmpty,
            Issue::Missing | Issue::ParentMissing => Message::UndoNewFolderMissing,
        };
        return (message, failure.path.clone());
    }
    if let CommandError::HistoryIdentity(failure) = error {
        let message = match failure.issue {
            oxyde_lib::models::error::HistoryIdentityIssue::Changed => {
                Message::UndoCopyIdentityChanged
            }
            oxyde_lib::models::error::HistoryIdentityIssue::Unverified => {
                Message::UndoCopyUnverified
            }
        };
        (message, failure.path.clone())
    } else {
        (Message::UndoFailed, format!("{error:?}"))
    }
}

pub fn redo_failure(error: &CommandError) -> (Message, String) {
    if let CommandError::HistoryRename(failure) = error {
        let message = match failure.issue {
            oxyde_lib::models::error::HistoryIdentityIssue::Changed => Message::RedoRenameChanged,
            oxyde_lib::models::error::HistoryIdentityIssue::Unverified => {
                Message::RedoRenameUnverified
            }
        };
        return (message, failure.path.clone());
    }
    if let CommandError::HistoryMove(failure) = error {
        use oxyde_lib::models::error::HistoryMoveIssue as Issue;
        let message = match failure.issue {
            Issue::Changed => Message::RedoMoveChanged,
            Issue::Conflict => Message::RedoMoveConflict,
            Issue::Unverified => Message::RedoMoveUnverified,
            Issue::Incomplete => Message::RedoMoveIncomplete,
        };
        return (message, failure.detail.clone());
    }

    if let CommandError::NewFolderHistory(failure) = error {
        use oxyde_lib::models::error::NewFolderHistoryIssue as Issue;
        let message = match failure.issue {
            Issue::Changed | Issue::Conflict => Message::RedoNewFolderConflict,
            Issue::Missing | Issue::ParentMissing => Message::RedoNewFolderParentMissing,
            Issue::Unverified | Issue::NotEmpty => Message::RedoNewFolderUnverified,
        };
        return (message, failure.path.clone());
    }
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
    fn undo_recovery_error_translates_locations_without_exposing_technical_details() {
        use crate::ui::i18n::{I18n, Language};
        use fluent_bundle::FluentArgs;
        use oxyde_lib::models::error::HistoryUndoRecoveryFailure;
        let locations = "C:\\Données_日本語\\{ $detail }\nD:\\Récupération\\output";
        let error = CommandError::from(std::io::Error::other(HistoryUndoRecoveryFailure {
            locations: locations.into(),
            technical_detail: "technical reason hidden from UI".into(),
        }));
        let (message, detail) = undo_failure(&error);
        assert_eq!(message, Message::UndoRecoveryIncomplete);
        assert_eq!(detail, locations);
        for language in Language::ALL {
            let mut args = FluentArgs::new();
            args.set("detail", detail.as_str());
            let rendered = I18n::for_language(language).text_with_args(message, &args);
            assert!(rendered.contains(locations), "{language:?}: {rendered}");
            assert!(!rendered.contains(message.key()));
            assert!(!rendered.contains("technical reason hidden from UI"));
        }
    }

    #[test]
    fn redo_move_errors_are_translated_and_keep_literal_paths_in_all_languages() {
        use crate::ui::i18n::{I18n, Language};
        use fluent_bundle::FluentArgs;
        use oxyde_lib::models::error::{HistoryMoveFailure, HistoryMoveIssue as Issue};
        let path = "C:\\Données_日本語\\{ $detail }";
        for (issue, expected) in [
            (Issue::Changed, Message::RedoMoveChanged),
            (Issue::Conflict, Message::RedoMoveConflict),
            (Issue::Unverified, Message::RedoMoveUnverified),
            (Issue::Incomplete, Message::RedoMoveIncomplete),
        ] {
            let error = CommandError::from(std::io::Error::other(HistoryMoveFailure {
                issue,
                detail: path.into(),
            }));
            let (message, detail) = redo_failure(&error);
            assert_eq!(message, expected);
            assert_eq!(detail, path);
            for language in Language::ALL {
                let mut args = FluentArgs::new();
                args.set("detail", detail.as_str());
                let rendered = I18n::for_language(language).text_with_args(message, &args);
                assert!(rendered.contains(path), "{language:?}: {rendered}");
                assert!(!rendered.contains(message.key()));
                assert!(!rendered.contains("Move history"));
            }
        }
    }

    #[test]
    fn new_folder_history_messages_preserve_literal_paths_in_every_language() {
        use crate::ui::i18n::{I18n, Language};
        use fluent_bundle::FluentArgs;
        use oxyde_lib::models::error::{NewFolderHistoryFailure, NewFolderHistoryIssue as Issue};
        let path = "C:\\Données_日本語\\{ $detail }";
        for (redo, issue, message) in [
            (false, Issue::Changed, Message::UndoNewFolderChanged),
            (false, Issue::Unverified, Message::UndoNewFolderUnverified),
            (false, Issue::NotEmpty, Message::UndoNewFolderNotEmpty),
            (false, Issue::Missing, Message::UndoNewFolderMissing),
            (true, Issue::Conflict, Message::RedoNewFolderConflict),
            (
                true,
                Issue::ParentMissing,
                Message::RedoNewFolderParentMissing,
            ),
            (true, Issue::Unverified, Message::RedoNewFolderUnverified),
        ] {
            let error = CommandError::from(std::io::Error::other(NewFolderHistoryFailure {
                issue,
                path: path.into(),
            }));
            let (actual, detail) = if redo {
                redo_failure(&error)
            } else {
                undo_failure(&error)
            };
            assert_eq!(actual, message);
            assert_eq!(detail, path);
            for language in Language::ALL {
                let mut args = FluentArgs::new();
                args.set("detail", detail.as_str());
                let rendered = I18n::for_language(language).text_with_args(actual, &args);
                assert!(rendered.contains(path), "{language:?}: {rendered}");
                assert!(!rendered.contains(message.key()));
                assert!(!rendered.contains("New-folder history"));
            }
        }
    }

    #[test]
    fn undo_identity_failures_are_translated_and_preserve_literal_paths() {
        use crate::ui::i18n::{I18n, Language};
        use fluent_bundle::FluentArgs;
        use oxyde_lib::models::error::{HistoryIdentityFailure, HistoryIdentityIssue};
        for path in [
            "C:\\école\\{ $detail }.txt",
            "C:\\dossier_日本語\\{ $detail }",
        ] {
            for (issue, message) in [
                (
                    HistoryIdentityIssue::Changed,
                    Message::UndoCopyIdentityChanged,
                ),
                (
                    HistoryIdentityIssue::Unverified,
                    Message::UndoCopyUnverified,
                ),
            ] {
                let error = CommandError::from(std::io::Error::other(HistoryIdentityFailure {
                    issue,
                    path: path.into(),
                }));
                let (actual, detail) = undo_failure(&error);
                assert_eq!(actual, message);
                assert_eq!(detail, path);
                for language in Language::ALL {
                    let mut args = FluentArgs::new();
                    args.set("detail", detail.as_str());
                    let rendered = I18n::for_language(language).text_with_args(actual, &args);
                    assert!(rendered.contains(path), "{language:?}: {rendered}");
                    assert!(!rendered.contains("History file identity"));
                    assert!(!rendered.contains(actual.key()));
                }
            }
        }
        assert_eq!(
            undo_failure(&CommandError::Other("unrelated".into())).0,
            Message::UndoFailed
        );
    }
    #[test]
    fn rename_identity_failures_are_translated_for_undo_and_redo() {
        use crate::ui::i18n::{I18n, Language};
        use fluent_bundle::FluentArgs;
        use oxyde_lib::models::error::{HistoryIdentityIssue, HistoryRenameFailure};
        let path = "C:/école_日本語/{ $detail }";
        for (issue, undo, redo) in [
            (
                HistoryIdentityIssue::Changed,
                Message::UndoRenameChanged,
                Message::RedoRenameChanged,
            ),
            (
                HistoryIdentityIssue::Unverified,
                Message::UndoRenameUnverified,
                Message::RedoRenameUnverified,
            ),
        ] {
            let error = CommandError::from(std::io::Error::other(HistoryRenameFailure {
                issue,
                path: path.into(),
            }));
            assert_eq!(undo_failure(&error), (undo, path.into()));
            assert_eq!(redo_failure(&error), (redo, path.into()));
            for language in Language::ALL {
                for message in [undo, redo] {
                    let mut args = FluentArgs::new();
                    args.set("detail", path);
                    let rendered = I18n::for_language(language).text_with_args(message, &args);
                    assert!(rendered.contains(path), "{language:?}: {rendered}");
                    assert!(!rendered.contains(message.key()));
                    assert!(!rendered.contains("Rename history"));
                }
            }
        }
    }
    #[test]
    fn move_identity_failures_are_translated_for_undo_and_redo() {
        use crate::ui::i18n::{I18n, Language};
        use fluent_bundle::FluentArgs;
        use oxyde_lib::models::error::{HistoryMoveFailure, HistoryMoveIssue};
        let path = "C:\\école_日本語\\{ $detail }.txt";
        for (issue, undo, redo) in [
            (
                HistoryMoveIssue::Changed,
                Message::UndoMoveChanged,
                Message::RedoMoveChanged,
            ),
            (
                HistoryMoveIssue::Unverified,
                Message::UndoMoveUnverified,
                Message::RedoMoveUnverified,
            ),
        ] {
            let error = CommandError::HistoryMove(HistoryMoveFailure {
                issue,
                detail: path.into(),
            });
            assert_eq!(undo_failure(&error), (undo, path.into()));
            assert_eq!(redo_failure(&error), (redo, path.into()));
            for language in Language::ALL {
                for message in [undo, redo] {
                    let mut args = FluentArgs::new();
                    args.set("detail", path);
                    let rendered = I18n::for_language(language).text_with_args(message, &args);
                    assert!(rendered.contains(path), "{language:?}: {rendered}");
                    assert!(!rendered.contains(message.key()));
                    assert!(!rendered.contains("History move"));
                }
            }
        }
    }
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
