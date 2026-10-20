//! Storage is selected once from the executable location, never the working directory.
use std::{io, path::{Path, PathBuf}, sync::OnceLock};

struct StoragePaths {
    config: Option<PathBuf>,
    cache: Option<PathBuf>,
    temporary: PathBuf,
    portable: bool,
}

static PATHS: OnceLock<io::Result<StoragePaths>> = OnceLock::new();

fn paths() -> io::Result<&'static StoragePaths> {
    PATHS.get_or_init(|| {
        StoragePaths::resolve(&std::env::current_exe()?, dirs::config_dir(),
            dirs::cache_dir(), std::env::temp_dir())
    }).as_ref().map_err(|error| io::Error::new(error.kind(), error.to_string()))
}

impl StoragePaths {
    fn resolve(executable: &Path, config: Option<PathBuf>, cache: Option<PathBuf>,
        temporary: PathBuf) -> io::Result<Self> {
        let data = executable.parent().ok_or_else(|| io::Error::other("Executable has no parent"))?.join("data");
        match std::fs::metadata(&data) {
            Ok(metadata) if metadata.is_dir() => Ok(Self {
                config: Some(data.clone()), cache: Some(data.join("cache")),
                temporary: data.join("temp"), portable: true,
            }),
            Ok(_) => Err(io::Error::other("Portable data path is not a directory")),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self {
                config: config.map(|root| root.join("Oxyde")),
                cache: cache.map(|root| root.join("Oxyde")), temporary, portable: false,
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
pub fn temporary_dir() -> io::Result<PathBuf> { Ok(paths()?.temporary.clone()) }

#[cfg(test)]
#[path = "app_paths/tests/storage.rs"]
mod tests;
