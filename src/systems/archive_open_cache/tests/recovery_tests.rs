use super::*;
use crate::systems::archive_owned_tree::OwnedTree;

fn abandoned(base: &Path, written: usize, complete: bool) -> PathBuf {
    let mut root = OwnedTree::temporary_prefix(base, "entry-").unwrap();
    let path = root.path().to_path_buf();
    root.ensure_directory(&path.join("data")).unwrap();
    let mut marker = root.create_file(&path.join(OWNER)).unwrap();
    marker.write_all(b"{\"version\":4,\"created\":1}").unwrap();
    drop(marker);
    let journal = Journal::new(&mut root, 1).unwrap();
    let mut payload = OwnedTree::new(&path.join("data"), false).unwrap();
    payload.attach_recovery(journal.clone());
    payload
        .ensure_directory(&payload.path().join("docs"))
        .unwrap();
    let file_path = payload.path().join("docs/document");
    let mut file = payload.create_file(&file_path).unwrap();
    let bytes = vec![255; crate::utils::archive_io_protocol::MAX_BLOCK];
    payload
        .before_cache_write(&file_path, &file, &bytes)
        .unwrap();
    file.write_all(&bytes[..written]).unwrap();
    file.sync_all().unwrap();
    if complete {
        assert_eq!(written, bytes.len());
        payload.complete_cache_write(&file_path, &file).unwrap();
        journal.lock().unwrap().finish().unwrap();
    }
    drop(file);
    payload.keep();
    root.keep();
    drop(payload);
    drop(root);
    drop(journal);
    path
}
fn reclaim(path: &Path) -> io::Result<bool> {
    evict(
        path,
        Policy::DEFAULT,
        120,
        true,
        &mut MAX_SCAN_NODES.clone(),
    )
}

#[test]
fn durable_expected_write_recovers_zero_partial_and_complete_payload_prefixes() {
    for written in [0, 3, crate::utils::archive_io_protocol::MAX_BLOCK] {
        let base = tempfile::tempdir().unwrap();
        let path = abandoned(
            base.path(),
            written,
            written == crate::utils::archive_io_protocol::MAX_BLOCK,
        );
        assert!(reclaim(&path).unwrap());
        assert!(!path.exists());
    }
}

#[test]
fn foreign_child_and_modified_or_replaced_payload_preserve_the_entire_entry() {
    for scenario in ["foreign", "modified", "replaced", "provenance"] {
        let base = tempfile::tempdir().unwrap();
        let path = abandoned(base.path(), 3, false);
        let payload = path.join("data/docs/document");
        match scenario {
            "foreign" => fs::write(path.join("foreign"), b"external document").unwrap(),
            "modified" => fs::write(&payload, b"new").unwrap(),
            "replaced" => {
                // Keep the original identity alive so this test cannot depend
                // on the filesystem's identifier reuse after deletion.
                fs::rename(&payload, path.join("original-moved")).unwrap();
                fs::write(&payload, [255; 3]).unwrap();
            }
            _ => fs::write(
                format!("{}:Zone.Identifier", payload.display()),
                b"[ZoneTransfer]\r\nZoneId=3\r\n",
            )
            .unwrap(),
        }
        assert!(!reclaim(&path).unwrap());
        assert!(payload.exists());
        assert!(path.join(OWNER).exists());
        if scenario == "foreign" {
            assert_eq!(
                fs::read(path.join("foreign")).unwrap(),
                b"external document"
            );
        }
    }
}

#[test]
fn forged_or_replayed_proofs_and_plain_owner_downgrades_grant_no_cleanup_authority() {
    let base = tempfile::tempdir().unwrap();
    let first = abandoned(base.path(), 3, false);
    let second = abandoned(base.path(), 3, false);
    for slot in SLOTS {
        fs::copy(first.join(slot), second.join(slot)).unwrap();
    }
    fs::write(second.join(OWNER), b"{\"version\":2,\"created\":1}").unwrap();
    assert!(!reclaim(&second).unwrap());
    for slot in SLOTS {
        let path = first.join(slot);
        let mut bytes = fs::read(&path).unwrap();
        let middle = bytes.len() / 2;
        bytes[middle] ^= 0x80;
        fs::write(path, bytes).unwrap();
    }
    assert!(!reclaim(&first).unwrap());
    assert!(first.join("data/docs/document").exists());
    assert!(second.join("data/docs/document").exists());
}

#[test]
fn active_handle_blocks_recovery_then_releases_it_without_adopting_unknown_data() {
    let base = tempfile::tempdir().unwrap();
    let path = abandoned(base.path(), 3, false);
    let reader = File::open(path.join("data/docs/document")).unwrap();
    assert!(reclaim(&path).is_err());
    assert!(path.join("data/docs/document").exists());
    drop(reader);
    assert!(reclaim(&path).unwrap());
}
