use super::*;
use crate::utils::{archive_limits::ArchiveLimits, tar_fidelity};
use std::io::{Seek, Write};

fn header(name: &str, kind: tar::EntryType, size: u64) -> tar::Header {
    let mut header = tar::Header::new_gnu();
    header.set_path(name).unwrap();
    header.set_entry_type(kind);
    header.set_size(size);
    header.set_mode(0o644);
    header.set_mtime(123);
    header
}

fn padded(output: &mut Vec<u8>, bytes: &[u8]) {
    output.extend_from_slice(bytes);
    output.resize(output.len().next_multiple_of(512), 0);
}

fn octal(field: &mut [u8], value: u64) {
    let text = format!("{value:0width$o}", width = field.len() - 1);
    field[..text.len()].copy_from_slice(text.as_bytes());
    field[text.len()..].fill(0);
}

/// Sparse blocks as (offset, bytes); the first four sit in the header, the
/// rest in one extension header, as GNU tar writes them.
fn sparse(output: &mut Vec<u8>, name: &str, real: u64, blocks: &[(u64, &[u8])]) {
    let stored: usize = blocks.iter().map(|(_, bytes)| bytes.len()).sum();
    let mut header = header(name, tar::EntryType::GNUSparse, stored as u64);
    let gnu = header.as_gnu_mut().unwrap();
    for (slot, (offset, bytes)) in gnu.sparse.iter_mut().zip(blocks) {
        octal(&mut slot.offset, *offset);
        octal(&mut slot.numbytes, bytes.len() as u64);
    }
    octal(&mut gnu.realsize, real);
    let mut extension = [0u8; 512];
    if blocks.len() > 4 {
        gnu.isextended[0] = 1;
        for (index, (offset, bytes)) in blocks[4..].iter().enumerate() {
            octal(&mut extension[index * 24..index * 24 + 12], *offset);
            octal(&mut extension[index * 24 + 12..index * 24 + 24], bytes.len() as u64);
        }
    }
    header.set_cksum();
    output.extend_from_slice(header.as_bytes());
    if blocks.len() > 4 {
        output.extend_from_slice(&extension);
    }
    let data: Vec<u8> = blocks.iter().flat_map(|(_, bytes)| bytes.iter().copied()).collect();
    padded(output, &data);
}

fn member(output: &mut Vec<u8>, name: &str, kind: tar::EntryType, bytes: &[u8]) {
    let mut header = header(name, kind, bytes.len() as u64);
    header.set_cksum();
    output.extend_from_slice(header.as_bytes());
    padded(output, bytes);
}

fn pax(output: &mut Vec<u8>, kind: tar::EntryType, records: &[(&str, &[u8])]) {
    let mut data = Vec::new();
    for (key, value) in records {
        let body = [b" ", key.as_bytes(), b"=", value, b"\n"].concat();
        let mut length = body.len() + 1;
        while (length.to_string().len() + body.len()) > length {
            length += 1;
        }
        data.extend_from_slice(length.to_string().as_bytes());
        data.extend_from_slice(&body);
    }
    let mut header = tar::Header::new_ustar();
    header.set_path("pax_header").unwrap();
    header.set_entry_type(kind);
    header.set_size(data.len() as u64);
    header.set_cksum();
    output.extend_from_slice(header.as_bytes());
    padded(output, &data);
}

fn file(mut bytes: Vec<u8>) -> tempfile::NamedTempFile {
    bytes.resize(bytes.len() + 1024, 0);
    let mut file = tempfile::NamedTempFile::new().unwrap();
    file.write_all(&bytes).unwrap();
    file.rewind().unwrap();
    file
}

/// Name, directory, size and BLAKE3 of each member as the tar crate reads it.
fn decoded(file: &mut std::fs::File) -> io::Result<Vec<(String, bool, u64, blake3::Hash)>> {
    file.rewind()?;
    let mut archive = tar::Archive::new(&*file);
    let mut found = Vec::new();
    for entry in members(archive.entries()?) {
        let mut entry = entry?;
        let directory = directory(&mut entry)?.ok_or_else(|| io::Error::other("link"))?;
        let name = entry.path()?.to_string_lossy().into_owned();
        let size = entry.size();
        let mut hash = blake3::Hasher::new();
        assert_eq!(io::copy(&mut entry, &mut hash)?, size);
        found.push((name, directory, size, hash.finalize()));
    }
    Ok(found)
}

fn verified(file: &mut std::fs::File) -> io::Result<Vec<(String, bool, u64, blake3::Hash)>> {
    file.rewind()?;
    Ok(tar_fidelity::scan(file, ArchiveLimits::WORKSPACE, None, true, &|| Ok(()))?
        .into_iter()
        .map(|record| {
            let name = record.name.to_string_lossy().into_owned();
            (name, record.directory, record.size, record.hash.unwrap())
        })
        .collect())
}

#[test]
fn git_archive_global_headers_contiguous_and_sparse_files_are_members() {
    let mut bytes = Vec::new();
    // `git archive` starts with the commit in a PAX global header.
    pax(&mut bytes, tar::EntryType::XGlobalHeader, &[("comment", b"0123abcd")]);
    member(&mut bytes, "project/", tar::EntryType::Directory, b"");
    member(&mut bytes, "project/readme", tar::EntryType::Regular, b"read me");
    member(&mut bytes, "project/contiguous", tar::EntryType::Continuous, b"contiguous");
    let (a, b, c, d) = ([b'a'; 512], [b'b'; 512], [b'c'; 512], [b'd'; 512]);
    sparse(&mut bytes, "project/short", 3000, &[(0, &a), (2048, &b[..100]), (3000, &[])]);
    sparse(
        &mut bytes,
        "project/extended",
        5000,
        &[(0, &a), (1024, &b), (2048, &c), (3072, &d), (4096, b"tail"), (5000, &[])],
    );
    let mut archive = file(bytes);
    let decoded = decoded(archive.as_file_mut()).unwrap();
    let mut short = vec![b'a'; 512];
    short.resize(2048, 0);
    short.extend_from_slice(&b[..100]);
    short.resize(3000, 0);
    let mut extended = Vec::new();
    for (offset, bytes) in [(0, &a[..]), (1024, &b), (2048, &c), (3072, &d), (4096, b"tail")] {
        extended.resize(offset, 0);
        extended.extend_from_slice(bytes);
    }
    extended.resize(5000, 0);
    let expected = vec![
        ("project/".to_owned(), true, 0, blake3::hash(b"")),
        ("project/readme".to_owned(), false, 7, blake3::hash(b"read me")),
        ("project/contiguous".to_owned(), false, 10, blake3::hash(b"contiguous")),
        ("project/short".to_owned(), false, 3000, blake3::hash(&short)),
        ("project/extended".to_owned(), false, 5000, blake3::hash(&extended)),
    ];
    assert_eq!(decoded, expected);
    // The independent verifier sees the same members with the same contents.
    let verified = verified(archive.as_file_mut()).unwrap();
    let names = |list: &[(String, bool, u64, blake3::Hash)]| -> Vec<_> {
        list.iter()
            .map(|(name, directory, size, hash)| {
                (name.replace('\\', "/").trim_end_matches('/').to_owned(), *directory, *size, *hash)
            })
            .collect()
    };
    assert_eq!(names(&verified), names(&decoded));
}

#[test]
fn pax_sparse_members_and_links_are_refused() {
    let mut bytes = Vec::new();
    pax(&mut bytes, tar::EntryType::XHeader, &[("GNU.sparse.size", b"4096")]);
    member(&mut bytes, "GNUSparseFile.0/file", tar::EntryType::Regular, b"map");
    assert!(decoded(file(bytes).as_file_mut()).is_err());
    let mut bytes = Vec::new();
    member(&mut bytes, "link", tar::EntryType::Symlink, b"");
    let mut archive = tar::Archive::new(file(bytes).reopen().unwrap());
    let mut entries = members(archive.entries().unwrap());
    assert_eq!(directory(&mut entries.next().unwrap().unwrap()).unwrap(), None);
}
