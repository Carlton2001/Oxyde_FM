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
        let mut value = Zeroizing::new(String::deserialize(deserializer)?);
        Self::new(std::mem::take(&mut *value)).ok_or_else(|| serde::de::Error::custom("invalid archive password"))
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

fn store() -> &'static Mutex<HashMap<PathBuf, ArchiveSecret>> {
    static STORE: OnceLock<Mutex<HashMap<PathBuf, ArchiveSecret>>> = OnceLock::new();
    STORE.get_or_init(Default::default)
}

/// The one password a worker process received for its request.
static WORKER: OnceLock<Option<ArchiveSecret>> = OnceLock::new();

/// Remembers `secret` for `archive` until Oxyde exits.
pub fn remember(archive: &Path, secret: ArchiveSecret) {
    if let Ok(mut store) = store().lock() {
        store.insert(archive.to_path_buf(), secret);
    }
}

pub fn forget(archive: &Path) {
    if let Ok(mut store) = store().lock() {
        store.remove(archive);
    }
}

/// The password to try for `archive`: the worker's own, or the one remembered.
pub fn for_archive(archive: &Path) -> Option<ArchiveSecret> {
    if let Some(secret) = WORKER.get() {
        return secret.clone();
    }
    store().lock().ok()?.get(archive).cloned()
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
