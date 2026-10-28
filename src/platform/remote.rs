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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteAnswer {
    /// `remember` keeps the password in the keyring, as GNOME's « Remember forever ».
    Login { user: String, domain: String, password: String, remember: bool },
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
