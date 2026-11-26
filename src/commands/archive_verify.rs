//! Checks the remembered password of an encrypted archive before the action it unlocks:
//! the smallest encrypted member is decoded, up to `SAMPLE` bytes. Runs in the
//! confined metadata worker, which receives the password on its input.
use crate::models::error::ArchivePasswordFailure;
use crate::models::CommandError;
use crate::utils::archive::ArchiveFormat;
use crate::utils::archive_password::{self, ArchiveSecret};
use std::fs::File;
use std::io::{self, BufReader, Read};
use std::path::Path;

/// Decoded bytes enough to refuse a wrong key; a smaller member is read whole, so
/// ZipCrypto's checksum also refuses the passwords passing its one-byte check.
const SAMPLE: u64 = 1024 * 1024;

/// `Ok` when the password opens the archive, or when it needs none; a damaged archive
/// is left to the action, which reports it.
pub(crate) fn verify_file(path: &Path, file: File) -> Result<(), CommandError> {
    let secret = archive_password::for_archive(path);
    let refusal = match ArchiveFormat::from_path(path) {
        Some(ArchiveFormat::Zip) => zip(file, secret.as_ref()),
        Some(ArchiveFormat::SevenZip) => seven(file, secret.as_ref()),
        Some(ArchiveFormat::Rar) => super::archive_rar::verify(path, file, SAMPLE),
        _ => None,
    };
    refusal.map_or(Ok(()), |failure| Err(failure.into()))
}

fn sample<R: Read + ?Sized>(reader: &mut R, size: u64) -> io::Result<()> {
    let read = io::copy(&mut reader.take(SAMPLE), &mut io::sink())?;
    if size <= SAMPLE && read != size {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Truncated member"));
    }
    Ok(())
}

/// Members whose key check is read besides the sampled one. A wrong ZipCrypto key
/// passes each member's one-byte check with odds of 1 in 256, independently: a stored
/// member larger than `SAMPLE` has no checksum read, but several checks still refuse.
const HEADER_CHECKS: usize = 8;

fn zip(file: File, secret: Option<&ArchiveSecret>) -> Option<ArchivePasswordFailure> {
    let mut archive = zip::ZipArchive::new(BufReader::new(file)).ok()?;
    let mut encrypted: Vec<_> = (0..archive.len())
        .filter_map(|index| archive.by_index_raw(index).ok().filter(|entry| entry.encrypted() && !entry.is_dir())
            .map(|entry| (entry.compressed_size(), index)))
        .collect();
    encrypted.sort_unstable();
    let (_, index) = *encrypted.first()?;
    let Some(secret) = secret else { return Some(ArchivePasswordFailure::Required) };
    // Opening a member reads its key check without decoding its data.
    for &(_, other) in encrypted.iter().skip(1).take(HEADER_CHECKS) {
        if let Some(failure) = archive.by_index_decrypt(other, secret.expose().as_bytes()).err()
            .and_then(|error| archive_password::zip_failure(&error))
        {
            return Some(failure);
        }
    }
    let mut entry = match archive.by_index_decrypt(index, secret.expose().as_bytes()) {
        Ok(entry) => entry,
        Err(error) => return archive_password::zip_failure(&error),
    };
    let size = entry.size();
    match sample(&mut entry, size) {
        // A ZipCrypto key passing its one-byte check by chance decrypts noise, which
        // fails the checksum or, when compressed, the decompression itself.
        Err(error)
            if error.kind() == io::ErrorKind::InvalidData
                || error.to_string().contains("Invalid checksum") =>
        {
            Some(ArchivePasswordFailure::Wrong)
        }
        _ => None,
    }
}

fn seven(file: File, secret: Option<&ArchiveSecret>) -> Option<ArchivePasswordFailure> {
    let tried = secret.is_some();
    let password = secret.map_or_else(sevenz_rust2::Password::empty, ArchiveSecret::seven);
    let mut source = BufReader::new(file);
    let archive = match sevenz_rust2::Archive::read(&mut source, &password) {
        Ok(archive) => archive,
        Err(error) => return archive_password::seven_failure(&error, tried),
    };
    // Every block of an archive made by 7-Zip shares its key: the smallest encrypted
    // one decides. A clear block would accept any password.
    let block = (0..archive.blocks.len())
        .filter(|block| {
            archive.blocks[*block].coders.iter().any(|coder| {
                coder.encoder_method_id() == sevenz_rust2::EncoderMethod::ID_AES256_SHA256
            })
        })
        .min_by_key(|block| archive.blocks[*block].get_unpack_size())?;
    let result = sevenz_rust2::BlockDecoder::new(1, block, &archive, &password, &mut source)
        .for_each_entries(&mut |entry, input| {
            sample(input, entry.size())?;
            Ok(false)
        });
    match result {
        Err(error) => archive_password::seven_failure(&error, tried),
        Ok(_) => None,
    }
}

#[cfg(test)]
#[path = "archive_verify/tests/verify_tests.rs"]
mod tests;
