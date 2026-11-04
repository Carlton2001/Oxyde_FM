//! Applications the desktop offers to open files with.
use std::path::PathBuf;

/// How well an application fits the files, as GNOME's application chooser sorts them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ApplicationFit {
    /// Declared for the file type, or used for it before.
    Recommended,
    /// Declared for a parent type, such as plain text for source code.
    Related,
    /// Any other installed application.
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationChoice {
    /// The desktop entry identifier, such as `org.gnome.TextEditor.desktop`.
    pub id: String,
    pub name: String,
    /// An image file of the theme, when one is found.
    pub icon: Option<PathBuf>,
    /// Opens these files when they are opened normally.
    pub is_default: bool,
    pub fit: ApplicationFit,
}
