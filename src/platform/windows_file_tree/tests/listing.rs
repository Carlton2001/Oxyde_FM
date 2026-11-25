use super::*;
use std::collections::BTreeSet;
use std::os::windows::fs::OpenOptionsExt;

fn open_directory(path: &Path) -> File {
    std::fs::OpenOptions::new().read(true).custom_flags(0x02000000).open(path).unwrap()
}

#[test]
fn listings_and_deletions_span_several_buffers() {
    let root = tempfile::tempdir().unwrap();
    let tree = root.path().join("tree");
    let mut expected = BTreeSet::new();
    let mut level = tree.clone();
    for depth in 0..3 {
        std::fs::create_dir(&level).unwrap();
        // About 130 bytes an entry: each level fills several 4 KiB buffers.
        for index in 0..300 {
            let name = format!("{depth}-{index:04}-{}", "x".repeat(48));
            std::fs::write(level.join(&name), b"").unwrap();
            if depth == 0 {
                expected.insert(OsString::from(name));
            }
        }
        level = level.join("sub");
        if depth == 0 {
            expected.insert(OsString::from("sub"));
        }
    }
    let mut listed = BTreeSet::new();
    enumerate(&open_directory(&tree), &mut |name| {
        assert!(listed.insert(name));
        Ok(())
    })
    .unwrap();
    assert_eq!(listed, expected);
    let parent = open_directory(root.path());
    remove(&parent, &tree, 1, &|| Ok(())).unwrap();
    assert!(!tree.exists());
}

#[test]
fn malformed_entries_are_refused() {
    let entry = |next: u32, length: u32, name: &[u16]| {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&next.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&length.to_le_bytes());
        for unit in name {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        bytes
    };
    let reader = open_directory(&std::env::temp_dir());
    let read = |bytes: Vec<u8>| {
        let mut names = Names::new(bytes.len().max(4));
        // SAFETY: the buffer holds at least bytes.len() bytes.
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), names.buffer.as_mut_ptr().cast::<u8>(), bytes.len()) };
        names.filled = bytes.len();
        names.offset = Some(0);
        names.first = false;
        let mut found = Vec::new();
        loop {
            match names.next(&reader) {
                Ok(Some(name)) => found.push(name),
                Ok(None) => return Ok(found),
                Err(error) => return Err(error),
            }
            if names.offset.is_none() {
                return Ok(found);
            }
        }
    };
    let a = [b'a' as u16];
    let mut two = entry(16, 2, &a);
    two.extend_from_slice(&[0, 0]);
    two.extend(entry(0, 2, &a));
    assert_eq!(read(two).unwrap(), [OsString::from("a"), OsString::from("a")]);
    // Odd, empty or overlong names.
    assert!(read(entry(0, 1, &a)).is_err());
    assert!(read(entry(0, 0, &[])).is_err());
    assert!(read(entry(0, 4, &a)).is_err());
    // A following entry inside this name, past the returned bytes, or truncated.
    assert!(read(entry(12, 2, &a)).is_err());
    assert!(read(entry(14, 2, &a)).is_err());
    let mut truncated = entry(16, 2, &a);
    truncated.extend_from_slice(&[0, 0, 1, 0]);
    assert!(read(truncated).is_err());
}
