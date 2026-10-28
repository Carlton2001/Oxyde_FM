//! Remote locations reached by URI, such as `smb://server/share`: Linux mounts them through
//! GVfs, as Nautilus does, and browses them at their local FUSE path.

/// What the server asks before mounting; `retry` follows a refused answer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RemotePrompt {
    pub message: String,
    pub user: String,
    pub domain: String,
    pub needs_user: bool,
    pub needs_domain: bool,
    pub needs_password: bool,
    pub anonymous: bool,
    pub saving: bool,
    pub retry: bool,
}

/// How long the password outlives the mount, as GNOME's mount dialog offers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PasswordSave {
    /// Forgotten once the share is unmounted.
    #[default]
    Never,
    /// Kept until the user logs out.
    Session,
    /// Kept in the user's keyring.
    Permanently,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteAnswer {
    Login { user: String, domain: String, password: String, save: PasswordSave },
    Anonymous,
    Cancel,
}

/// Where a connected location is browsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteLocation {
    /// A folder of the mounted share, at its local path.
    Local(std::path::PathBuf),
    /// A server or network list, enumerated by URI.
    Browse(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteError {
    Unsupported,
    /// The user dismissed the credentials prompt: nothing to report.
    Cancelled,
    Other(String),
}

impl std::fmt::Display for RemoteError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported => formatter.write_str("Remote locations are unavailable"),
            Self::Cancelled => formatter.write_str("Cancelled"),
            Self::Other(detail) => formatter.write_str(detail),
        }
    }
}
