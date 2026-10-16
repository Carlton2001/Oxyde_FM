use super::*;
use std::{fs::FileTimes, time::Duration};

#[test]
fn listing_captures_submillisecond_revisions_without_changing_display_dates() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("image.png");
    std::fs::write(&path, b"same length").unwrap();
    let base = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
    file.set_times(FileTimes::new().set_modified(base + Duration::from_micros(100)))
        .unwrap();
    let before = get_file_entry_from_path(&path).unwrap();
    file.set_times(FileTimes::new().set_modified(base + Duration::from_micros(200)))
        .unwrap();
    let after = get_file_entry_from_path(&path).unwrap();
    assert_eq!(before.modified, after.modified);
    assert_eq!(before.size, after.size);
    assert_ne!(before.content_stamp, after.content_stamp);
    assert_eq!(
        after.content_stamp,
        FileStamp::from_metadata(&file.metadata().unwrap())
    );
}
