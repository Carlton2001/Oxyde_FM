use crate::models::CommandError;
use std::path::{Path, PathBuf};

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

/// Joins a user-supplied path fragment to a base path without claiming confinement.
///
/// This is intentionally not a security boundary: absolute paths and `..` components
/// retain the platform's normal `Path::join` semantics. Use canonicalized containment
/// checks separately if an operation is ever restricted to a sandbox.
pub fn join_path(base: &Path, input: &str) -> PathBuf {
    base.join(input)
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
    fn test_join_path_windows() {
        let base = PathBuf::from("C:\\Base");

        let joined = join_path(&base, "sub/file.txt");
        assert_eq!(joined, PathBuf::from("C:\\Base\\sub/file.txt"));
    }

    #[test]
    fn test_join_path_keeps_parent_components() {
        let base = PathBuf::from("/base");
        let joined = join_path(&base, "../outside/file.txt");
        assert_eq!(joined, PathBuf::from("/base/../outside/file.txt"));
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
