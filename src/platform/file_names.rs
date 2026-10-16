use std::ffi::{OsStr, OsString};

/// Cached rules of a retained directory; normalization itself performs no I/O.
#[derive(Clone, Copy)]
pub struct FileNameRules {
    normalize: fn(&OsStr) -> OsString,
}

impl FileNameRules {
    pub(super) fn sensitive() -> Self {
        Self {
            normalize: OsStr::to_os_string,
        }
    }

    #[cfg(target_os = "windows")]
    pub(super) fn windows_insensitive() -> Self {
        Self {
            normalize: super::windows_file_names::fold,
        }
    }

    pub(crate) fn key(self, name: &OsStr) -> OsString {
        (self.normalize)(name)
    }
}
