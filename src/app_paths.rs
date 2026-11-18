//! Storage is selected once from the executable location, never the working directory.
use std::{io, path::{Path, PathBuf}, sync::OnceLock};

struct StoragePaths {
    config: Option<PathBuf>,
    cache: Option<PathBuf>,
    temporary: PathBuf,
    /// The system's temporary folder, which portable mode replaces.
    system_temporary: PathBuf,
    portable: bool,
}

static PATHS: OnceLock<io::Result<StoragePaths>> = OnceLock::new();

fn paths() -> io::Result<&'static StoragePaths> {
    PATHS.get_or_init(|| {
        StoragePaths::resolve(&real_path(&std::env::current_exe()?), dirs::config_dir().map(|path| real_path(&path)),
            dirs::cache_dir().map(|path| real_path(&path)), real_path(&std::env::temp_dir()))
    }).as_ref().map_err(|error| io::Error::new(error.kind(), error.to_string()))
}

/// The path with every link resolved, or unchanged when it cannot be. For the
/// application's own roots: they sit in the user's own profile or install
/// folder (a moved profile, a Scoop `current` junction, a linked `~/.cache`),
/// and later lookups refuse every link below them.
pub fn real_path(path: &Path) -> PathBuf {
    match std::fs::canonicalize(path) {
        Ok(real) => without_verbatim_prefix(real),
        Err(_) => path.to_path_buf(),
    }
}

/// `\\?\C:\…` as `C:\…` when both name the same file.
#[cfg(windows)]
fn without_verbatim_prefix(path: PathBuf) -> PathBuf {
    let plain = match path.to_str().and_then(|text| text.strip_prefix(r"\\?\")) {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') && rest.len() < 260 => PathBuf::from(rest),
        _ => return path,
    };
    // Trailing dots and spaces or device names would read differently.
    if std::path::absolute(&plain).is_ok_and(|absolute| absolute == plain) { plain } else { path }
}

#[cfg(not(windows))]
fn without_verbatim_prefix(path: PathBuf) -> PathBuf { path }

impl StoragePaths {
    fn resolve(executable: &Path, config: Option<PathBuf>, cache: Option<PathBuf>,
        temporary: PathBuf) -> io::Result<Self> {
        let data = executable.parent().ok_or_else(|| io::Error::other("Executable has no parent"))?.join("data");
        match std::fs::metadata(&data) {
            Ok(metadata) if metadata.is_dir() => {
                // A linked `data` folder (Scoop `persist`) is used where it lives.
                let data = real_path(&data);
                Ok(Self {
                    config: Some(data.clone()), cache: Some(data.join("cache")),
                    temporary: data.join("temp"), system_temporary: temporary, portable: true,
                })
            }
            Ok(_) => Err(io::Error::other("Portable data path is not a directory")),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self {
                config: config.map(|root| root.join("Oxyde")),
                cache: cache.map(|root| root.join("Oxyde")), system_temporary: temporary.clone(), temporary,
                portable: false,
            }),
            Err(error) => Err(error),
        }
    }

    fn prepare(&self) -> io::Result<()> {
        if self.portable {
            // Refuse an unwritable portable folder; never redirect writes to the user profile.
            let config = self.config.as_ref().expect("Portable configuration root");
            let probe = tempfile::NamedTempFile::new_in(config)?;
            probe.close()?;
            std::fs::create_dir_all(&self.temporary)?;
        }
        Ok(())
    }
}

/// Call before starting any runtime or background thread. Children inherit the local temp path.
pub fn initialize() -> io::Result<()> {
    let paths = paths()?;
    paths.prepare()?;
    if paths.portable {
        for name in ["TEMP", "TMP", "TMPDIR"] {
            std::env::set_var(name, &paths.temporary);
        }
    }
    Ok(())
}

pub fn config_dir() -> Option<PathBuf> { paths().ok()?.config.clone() }
pub fn cache_dir() -> Option<PathBuf> { paths().ok()?.cache.clone() }
/// True when a `data` folder beside the executable holds all application files.
pub fn is_portable() -> bool { paths().is_ok_and(|paths| paths.portable) }
pub fn temporary_dir() -> io::Result<PathBuf> { Ok(paths()?.temporary.clone()) }
/// The system's temporary folder, even in portable mode.
pub fn system_temporary_dir() -> io::Result<PathBuf> { Ok(paths()?.system_temporary.clone()) }

#[cfg(test)]
#[path = "app_paths/tests/storage.rs"]
mod tests;
