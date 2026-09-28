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
}
