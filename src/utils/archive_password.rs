//! Passwords of encrypted archives. The parent keeps each one in memory for its archive
//! until Oxyde exits; a worker receives the one for its request on its standard input,
//! never in its arguments. Nothing here is written to configuration or logs.
use crate::models::error::ArchivePasswordFailure;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use zeroize::Zeroizing;

/// Longest accepted password, in UTF-8 bytes.
pub const MAX_BYTES: usize = 1024;

/// A password whose `Debug` hides its value; its memory is cleared on drop.
#[derive(Clone, PartialEq, Eq)]
pub struct ArchiveSecret(Zeroizing<String>);

impl Serialize for ArchiveSecret {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.expose())
    }
}

impl<'de> Deserialize<'de> for ArchiveSecret {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // A refused value is still wiped when it drops here.
        let value = Zeroizing::new(String::deserialize(deserializer)?);
        if value.is_empty() || value.len() > MAX_BYTES {
            return Err(serde::de::Error::custom("invalid archive password"));
        }
        Ok(Self(value))
    }
}

impl ArchiveSecret {
    pub fn new(value: String) -> Option<Self> {
        (!value.is_empty() && value.len() <= MAX_BYTES).then(|| Self(Zeroizing::new(value)))
    }
    pub fn expose(&self) -> &str {
        &self.0
    }
    pub fn validate(&self) -> io::Result<()> {
        if self.0.is_empty() || self.0.len() > MAX_BYTES {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid archive password"));
        }
        Ok(())
    }
    pub(crate) fn seven(&self) -> sevenz_rust2::Password {
        sevenz_rust2::Password::from(self.expose())
    }
}

impl std::fmt::Debug for ArchiveSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ArchiveSecret(..)")
    }
}

/// Each password under its archive's key, with the key it was remembered under.
type Store = HashMap<PathBuf, (ArchiveSecret, PathBuf)>;

fn store() -> &'static Mutex<Store> {
    static STORE: OnceLock<Mutex<Store>> = OnceLock::new();
    STORE.get_or_init(Default::default)
}

/// The one password a worker process received for its request.
static WORKER: OnceLock<Option<ArchiveSecret>> = OnceLock::new();

/// The archive as the store knows it. Operations open a canonical path, which Windows
/// writes `\\?\C:\…`, while the dialog names the displayed one: both find the same entry.
fn key(archive: &Path) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        let text = archive.to_string_lossy().replace('/', r"\");
        let text = match text.strip_prefix(r"\\?\UNC\") {
            Some(share) => format!(r"\\{share}"),
            None => text.strip_prefix(r"\\?\").map_or_else(|| text.clone(), str::to_owned),
        };
        PathBuf::from(text.to_lowercase())
    }
    #[cfg(not(target_os = "windows"))]
    archive.to_path_buf()
}

/// Remembers `secret` for `archive` until Oxyde exits, under the displayed path
/// and the resolved one operations open: a mapped drive's share, a SUBST
/// target, links followed. Resolving reads the disk: never on the UI thread.
pub fn remember(archive: &Path, secret: ArchiveSecret) {
    let resolved = std::fs::canonicalize(archive).ok();
    remember_as(archive, resolved.as_deref(), secret);
}

fn remember_as(archive: &Path, resolved: Option<&Path>, secret: ArchiveSecret) {
    let origin = key(archive);
    let resolved = resolved.map(key);
    if let Ok(mut store) = store().lock() {
        forget_in(&mut store, &origin);
        if let Some(resolved) = resolved.filter(|resolved| *resolved != origin) {
            store.insert(resolved, (secret.clone(), origin.clone()));
        }
        store.insert(origin.clone(), (secret, origin));
    }
}

/// Forgets the password found for `archive` under every path it was remembered.
pub fn forget(archive: &Path) {
    if let Ok(mut store) = store().lock() {
        forget_in(&mut store, &key(archive));
    }
}

fn forget_in(store: &mut Store, key: &Path) {
    if let Some((_, origin)) = store.remove(key) {
        store.retain(|_, (_, other)| *other != origin);
    }
}

/// The password to try for `archive`: the worker's own, or the one remembered.
pub fn for_archive(archive: &Path) -> Option<ArchiveSecret> {
    if let Some(secret) = WORKER.get() {
        return secret.clone();
    }
    store().lock().ok()?.get(&key(archive)).map(|(secret, _)| secret.clone())
}

/// Parent side: sent once after the start gate, empty without a password.
pub fn send(output: &mut impl Write, secret: Option<&ArchiveSecret>) -> io::Result<()> {
    let bytes = secret.map_or(&b""[..], |secret| secret.expose().as_bytes());
    let mut frame = Zeroizing::new(Vec::with_capacity(2 + bytes.len()));
    frame.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
    frame.extend_from_slice(bytes);
    output.write_all(&frame)?;
    output.flush()
}

/// Worker side: reads the parent's frame and makes it the process's only password.
pub fn receive(input: &mut impl Read) -> io::Result<()> {
    let secret = read_frame(input)?;
    WORKER
        .set(secret)
        .map_err(|_| io::Error::other("Archive password already received"))
}

fn read_frame(input: &mut impl Read) -> io::Result<Option<ArchiveSecret>> {
    let mut length = [0; 2];
    input.read_exact(&mut length)?;
    let length = u16::from_le_bytes(length) as usize;
    if length > MAX_BYTES {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Archive password too long"));
    }
    let mut bytes = Zeroizing::new(vec![0; length]);
    input.read_exact(&mut bytes)?;
    if length == 0 {
        return Ok(None);
    }
    std::str::from_utf8(&bytes)
        .ok()
        .and_then(|text| ArchiveSecret::new(text.to_owned()))
        .map(Some)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Invalid archive password"))
}

/// A ZIP refusal caused by encryption, or `None` for any other error.
pub(crate) fn zip_failure(error: &zip::result::ZipError) -> Option<ArchivePasswordFailure> {
    use zip::result::ZipError;
    match error {
        ZipError::InvalidPassword => Some(ArchivePasswordFailure::Wrong),
        ZipError::UnsupportedArchive(detail) if *detail == ZipError::PASSWORD_REQUIRED => {
            Some(ArchivePasswordFailure::Required)
        }
        _ => None,
    }
}

/// A 7z refusal caused by encryption. Without a password, a bad-password guess means
/// the content needs one.
pub(crate) fn seven_failure(error: &sevenz_rust2::Error, tried: bool) -> Option<ArchivePasswordFailure> {
    match error {
        sevenz_rust2::Error::PasswordRequired => Some(ArchivePasswordFailure::Required),
        sevenz_rust2::Error::MaybeBadPassword(_) if tried => Some(ArchivePasswordFailure::Wrong),
        sevenz_rust2::Error::MaybeBadPassword(_) => Some(ArchivePasswordFailure::Required),
        _ => None,
    }
}

#[cfg(test)]
#[path = "archive_password/tests/password_tests.rs"]
mod tests;
