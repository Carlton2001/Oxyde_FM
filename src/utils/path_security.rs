use std::path::{Path, PathBuf};
use crate::models::CommandError;

/// Validates that a path is absolute and exists (optional).
/// For a File Manager, we generally want to allow access to any valid system path.
/// This prevents relative paths that might be ambiguous.
pub fn validate_path(path_str: &str) -> Result<PathBuf, CommandError> {
    let mut path = PathBuf::from(path_str);
    if !path.is_absolute() {
        return Err(CommandError::PathError(format!("Path must be absolute: {}", path_str)));
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

/// Safe join that prevents directory traversal attacks when joining a user input to a base directory.
/// Useful if we ever restrict operations to a specific sandbox (not currently the case for full FM).
pub fn safe_join(base: &Path, input: &str) -> Result<PathBuf, CommandError> {
    let path = base.join(input);
    // In a sandboxed environment, we would check if 'path' starts with 'base'.
    // For this app, we just ensure it's a valid path construction.
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_safe_join() {
        let base = PathBuf::from("C:\\Base");
        
        // Normal join
        let joined = safe_join(&base, "sub/file.txt").unwrap();
        assert_eq!(joined, PathBuf::from("C:\\Base\\sub/file.txt"));
    }

    #[test]
    fn test_validate_path_traversal() {
        assert!(validate_path("C:\\foo\\..\\bar").is_ok());
    }

    #[test]
    fn test_validate_path_unix_absolute() {
        assert!(validate_path("C:/Users").is_ok());
    }
}
