use super::*;
use std::fs;
use std::io::{Read, Seek};

fn copied(content: &[u8], shared_output: bool) -> (tempfile::TempDir, std::path::PathBuf, CopiedFile) {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let destination = root.path().join("destination");
    fs::write(&source, content).unwrap();
    let parents = super::super::path_parents::HeldParents::new(&destination).unwrap();
    let output = copy_with_parents_observed(&source, &destination, false, None, None, parents, &mut |_| Ok(()), None, shared_output).unwrap();
    (root, destination, output)
}

#[test]
fn a_copy_is_verified_through_its_creation_handle_which_shares_nothing() {
    // Several hash buffers, read by position from the asynchronous handle.
    let content: Vec<u8> = (0..300 * 1024).map(|index| (index % 251) as u8).collect();
    let (_root, destination, output) = copied(&content, false);
    assert_eq!(output.hash, blake3::hash(&content).to_hex().to_string());
    assert_eq!(fs::read(&destination).unwrap_err().raw_os_error(), Some(32));
    drop(output);
    assert_eq!(fs::read(&destination).unwrap(), content);
}

#[test]
fn an_empty_copy_is_verified_through_its_creation_handle() {
    let (_root, destination, output) = copied(b"", false);
    assert_eq!(output.hash, blake3::hash(b"").to_hex().to_string());
    drop(output);
    assert_eq!(fs::read(&destination).unwrap(), b"");
}

#[test]
fn a_shared_output_lets_readers_in_and_reads_in_order() {
    let (_root, destination, mut output) = copied(b"compressed later", true);
    assert_eq!(fs::read(&destination).unwrap(), b"compressed later");
    // As compression takes it, from its start.
    output.file.rewind().unwrap();
    let mut content = Vec::new();
    output.file.read_to_end(&mut content).unwrap();
    assert_eq!(content, b"compressed later");
}
