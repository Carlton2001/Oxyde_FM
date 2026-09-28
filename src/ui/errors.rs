use oxyde_lib::models::CommandError;

/// Converts backend failures into concise, localized messages suitable for the UI.
/// Technical details remain available in logs instead of leaking OS error strings to users.
pub fn path_load_message(error: &CommandError) -> String {
    match error {
        CommandError::PathNotFound(_) => {
            "Ce dossier est introuvable ou n’est plus disponible.".to_string()
        }
        CommandError::AccessDenied(_) => {
            "L’accès à ce dossier est refusé. Vérifiez vos autorisations.".to_string()
        }
        _ => "Impossible d’ouvrir ce dossier. Vérifiez le chemin et réessayez.".to_string(),
    }
}

pub fn rename_message(error: &CommandError) -> String {
    match error {
        CommandError::IoError(message)
            if message.to_ascii_lowercase().contains("already exists") =>
        {
            "Un fichier ou un dossier portant déjà ce nom existe ici.".to_string()
        }
        CommandError::PathNotFound(_) => {
            "L’élément à renommer n’existe plus ou n’est plus accessible.".to_string()
        }
        CommandError::AccessDenied(_) => {
            "Le renommage est refusé. Vérifiez les autorisations de cet élément.".to_string()
        }
        CommandError::IoError(_) | CommandError::PathError(_) => {
            "Le renommage a échoué. Vérifiez le nom et réessayez.".to_string()
        }
        _ => "Impossible de renommer cet élément pour le moment.".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_errors_are_localized_without_operating_system_details() {
        assert_eq!(
            path_load_message(&CommandError::PathNotFound("E:\\missing".into())),
            "Ce dossier est introuvable ou n’est plus disponible."
        );
        assert_eq!(
            path_load_message(&CommandError::AccessDenied("C:\\private".into())),
            "L’accès à ce dossier est refusé. Vérifiez vos autorisations."
        );
        assert_eq!(
            path_load_message(&CommandError::IoError("os error 3".into())),
            "Impossible d’ouvrir ce dossier. Vérifiez le chemin et réessayez."
        );
    }

    #[test]
    fn rename_conflicts_are_localized_without_os_error_details() {
        assert_eq!(
            rename_message(&CommandError::IoError(
                "Rename target already exists: D:\\Tests\\2".to_string()
            )),
            "Un fichier ou un dossier portant déjà ce nom existe ici."
        );
    }
}
