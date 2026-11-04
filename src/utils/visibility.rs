/// Listing and search use the same visibility contract: System takes precedence
/// over Hidden when both attributes are set. Names alone never imply System.
pub(crate) fn is_visible(hidden: bool, system: bool, show_hidden: bool, show_system: bool) -> bool {
    if system {
        show_system
    } else {
        show_hidden || !hidden
    }
}

/// Attributes of the entries of one directory, including the names its own list hides.
pub(crate) struct DirectoryAttributes {
    listed: std::collections::HashSet<String>,
}

impl DirectoryAttributes {
    pub(crate) fn load(directory: &std::path::Path) -> Self {
        Self { listed: crate::platform::platform().listed_hidden_names(directory) }
    }

    /// Hidden, system and reparse point flags, as `get_file_attributes` returns them.
    pub(crate) fn of(&self, metadata: &std::fs::Metadata, name: &str) -> (bool, bool, bool) {
        let (hidden, system, reparse) = super::get_file_attributes(metadata, name);
        (hidden || self.listed.contains(name), system, reparse)
    }
}

/// Loads each directory's list once during a recursive walk.
#[derive(Default)]
pub(crate) struct WalkAttributes {
    directories: std::collections::HashMap<std::path::PathBuf, DirectoryAttributes>,
}

impl WalkAttributes {
    pub(crate) fn of(&mut self, path: &std::path::Path, metadata: &std::fs::Metadata, name: &str) -> (bool, bool, bool) {
        let parent = path.parent().unwrap_or(path);
        if !self.directories.contains_key(parent) {
            self.directories.insert(parent.to_path_buf(), DirectoryAttributes::load(parent));
        }
        self.directories[parent].of(metadata, name)
    }
}

#[cfg(test)]
#[path = "visibility/tests/attributes.rs"]
mod tests;
