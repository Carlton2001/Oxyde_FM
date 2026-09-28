use crate::ui::i18n::Message;
use oxyde_lib::models::CommandError;

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
