//! One encoded thumbnail per cover, plus a small revision-keyed reference per song.
use super::{publish_cache_bytes, MAX_BYTES};
use std::{io::{Read, Result}, path::Path};

const MAGIC: &[u8; 8] = b"OXYDacr1";

pub(super) fn resolve(source: &Path, bytes: Vec<u8>, limit: usize) -> Option<Vec<u8>> {
    if !bytes.starts_with(MAGIC) { return Some(bytes); }
    let digest: [u8; 32] = bytes.get(8..)?.try_into().ok()?;
    let digest = blake3::Hash::from(digest);
    // Only a hash is stored: a reference can never redirect outside its cache directory.
    let path = source.with_file_name(format!("{}.jpg", digest.to_hex()));
    let file = oxyde_lib::platform::platform().read_cache_file(&path).ok()?;
    let mut pixels = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut pixels).ok()?;
    (pixels.len() <= limit && blake3::hash(&pixels) == digest).then_some(pixels)
}

pub(super) fn publish(source: &Path, bytes: &[u8], valid: impl Fn() -> bool) -> Result<()> {
    let digest = blake3::hash(bytes);
    let path = source.with_file_name(format!("{}.jpg", digest.to_hex()));
    let mut reference = MAGIC.to_vec();
    reference.extend_from_slice(digest.as_bytes());
    // Verify the existing blob rather than trusting its name or a previous write.
    if resolve(source, reference.clone(), MAX_BYTES).as_deref() != Some(bytes) {
        publish_cache_bytes(&path, bytes)?;
    }
    if valid() { publish_cache_bytes(source, &reference)?; }
    Ok(())
}
