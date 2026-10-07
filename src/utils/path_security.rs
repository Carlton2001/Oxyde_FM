use crate::models::CommandError;
use std::path::PathBuf;

/// Validates that a path is absolute and exists (optional).
/// For a File Manager, we generally want to allow access to any valid system path.
/// This prevents relative paths that might be ambiguous.
pub fn validate_path(path_str: &str) -> Result<PathBuf, CommandError> {
    let mut path = PathBuf::from(path_str);
    if !path.is_absolute() {
        return Err(CommandError::PathError(format!(
            "Path must be absolute: {}",
            path_str
        )));
    }

    #[cfg(target_os = "windows")]
    {
        let mut needs_update = None;
        if let Some(file_name) = path.file_name() {
            let name_str = file_name.to_string_lossy();
            let trimmed_name = name_str.trim_end_matches(['.', ' ']);
            if (!trimmed_name.is_empty() || name_str.is_empty()) && trimmed_name != name_str {
                needs_update = Some(trimmed_name.to_string());
            }
        }
        if let Some(new_name) = needs_update {
            path.set_file_name(new_name);
        }
    }

    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "windows")]
    #[test]
    fn test_validate_path_valid_absolute() {
        // Windows style
        assert!(validate_path("C:\\Users\\Test").is_ok());
        assert!(validate_path("D:/Data/Files").is_ok());
    }

    #[test]
    fn test_validate_path_invalid_relative() {
        let result = validate_path("relative/path");
        assert!(result.is_err());
        if let Err(CommandError::PathError(msg)) = result {
            assert!(msg.contains("Path must be absolute"));
        } else {
            panic!("Expected PathError");
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_validate_path_traversal() {
        assert!(validate_path("C:\\foo\\..\\bar").is_ok());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_validate_path_drive_absolute() {
        assert!(validate_path("C:/Users").is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn test_validate_path_unix_absolute() {
        assert!(validate_path("/Users").is_ok());
    }
}
