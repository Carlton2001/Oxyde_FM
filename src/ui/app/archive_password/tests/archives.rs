use super::transfer_archive;
use std::path::PathBuf;

#[test]
fn the_archive_entered_wins_over_an_archive_only_copied() {
    let directory = tempfile::tempdir().unwrap();
    let copied = directory.path().join("copied.zip");
    let opened = directory.path().join("opened.zip");
    for archive in [&copied, &opened] {
        let mut writer = zip::ZipWriter::new(std::fs::File::create(archive).unwrap());
        writer.start_file("member.txt", zip::write::SimpleFileOptions::default()).unwrap();
        writer.finish().unwrap();
    }
    let folder = PathBuf::from(directory.path());
    // Out of an archive, into one (its root or a folder inside), then an extraction.
    assert_eq!(transfer_archive(&[copied.clone(), opened.join("member.txt")], Some(&folder)), Some(opened.clone()));
    assert_eq!(transfer_archive(&[copied.clone()], Some(&opened)), Some(opened.clone()));
    assert_eq!(transfer_archive(&[copied.clone()], Some(&opened.join("inner"))), Some(opened));
    assert_eq!(transfer_archive(&[copied.clone()], Some(&folder)), Some(copied));
    assert_eq!(transfer_archive(&[folder.join("plain.txt")], Some(&folder)), None);
}
