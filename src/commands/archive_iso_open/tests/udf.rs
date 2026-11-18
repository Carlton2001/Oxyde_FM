use super::*;
use crate::commands::archive::test_iso_image;

/// The fixture's terminator is sector 17; its path tables, which the reader
/// does not use, lie where the extended area starts.
fn image_with(descriptors: &[&[u8; 5]]) -> (tempfile::TempDir, PathBuf) {
    let folder = tempfile::tempdir().unwrap();
    let archive = folder.path().join("disc.iso");
    test_iso_image(&archive);
    let mut bytes = std::fs::read(&archive).unwrap();
    for (index, identifier) in descriptors.iter().enumerate() {
        let offset = (18 + index) * 2048;
        bytes[offset] = 0;
        bytes[offset + 1..offset + 6].copy_from_slice(*identifier);
        bytes[offset + 6] = 1;
    }
    std::fs::write(&archive, bytes).unwrap();
    (folder, archive)
}

#[test]
fn udf_bridge_images_are_refused_so_the_system_mounts_them() {
    let (_folder, archive) = image_with(&[b"BEA01", b"NSR02"]);
    assert!(list_iso(&archive, "").is_err());
}

#[test]
fn an_extended_area_without_udf_keeps_the_iso_9660_tree() {
    let (_folder, archive) = image_with(&[b"BEA01", b"TEA01"]);
    let names: Vec<_> = list_iso(&archive, "").unwrap().into_iter().map(|entry| entry.name).collect();
    assert!(names.iter().any(|name| name == "ROOT.TXT"), "{names:?}");
}
