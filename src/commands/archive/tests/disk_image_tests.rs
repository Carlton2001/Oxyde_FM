use super::*;
fn malformed_iso_rejected_without_panic(record_byte: usize, value: u8) {
    let temp = tempfile::tempdir().unwrap();
    let image = temp.path().join("invalid.iso");
    test_iso_image(&image);
    let mut bytes = fs::read(&image).unwrap();
    let mut offset = 20 * 2048;
    for _ in 0..3 {
        offset += bytes[offset] as usize;
    }
    bytes[offset + record_byte] = value;
    fs::write(&image, bytes).unwrap();
    let output = temp.path().join("output");
    fs::create_dir(&output).unwrap();
    let listing = std::panic::catch_unwind(|| {
        list_archive_contents(image.to_string_lossy().into_owned(), String::new())
    });
    let extraction = std::panic::catch_unwind(|| {
        extract_archive_with_checkpoint(
            image.to_string_lossy().into_owned(),
            output.to_string_lossy().into_owned(),
            &|| Ok(()),
        )
    });
    assert!(listing.is_ok(), "listing panicked on a malformed ISO");
    assert!(extraction.is_ok(), "extraction panicked on a malformed ISO");
    assert!(listing.unwrap().is_err(), "invalid ISO listing succeeded");
    assert!(
        extraction.unwrap().is_err(),
        "incomplete ISO extraction succeeded"
    );
    assert!(fs::read_dir(output).unwrap().next().is_none());
}

#[test]
fn iso_invalid_identifier_returns_error_instead_of_panicking() {
    malformed_iso_rejected_without_panic(32, 255);
}

#[test]
fn iso_invalid_record_is_not_a_successful_end_of_directory() {
    malformed_iso_rejected_without_panic(0, 20);
}

#[test]
fn iso_navigation_extraction_and_cancellation() {
    let temp = tempfile::tempdir().unwrap();
    let image = temp.path().join("disc.ISO");
    test_iso_image(&image);
    let root =
        list_archive_contents(image.to_string_lossy().into_owned(), String::new()).unwrap();
    assert_eq!(root.len(), 2);
    let docs = root.iter().find(|entry| entry.name == "DOCS").unwrap();
    assert!(docs.is_dir);
    let nested =
        list_archive_contents(image.to_string_lossy().into_owned(), "DOCS".into()).unwrap();
    assert_eq!(
        nested[0].path,
        image.join("DOCS").join("README.TXT").to_string_lossy()
    );
    let workspace =
        crate::systems::archive_workspace::ArchiveWorkspace::open(image.clone()).unwrap();
    assert_eq!(
        fs::read(workspace.path("DOCS/README.TXT").unwrap()).unwrap(),
        b"ISO content"
    );
    assert!(!crate::systems::archive_workspace::writable(&image));
    let output = temp.path().join("cancelled");
    fs::create_dir(&output).unwrap();
    let checkpoints = std::cell::Cell::new(0);
    assert!(extract_archive_with_checkpoint(
        image.to_string_lossy().into_owned(),
        output.to_string_lossy().into_owned(),
        &|| {
            let count = checkpoints.get() + 1;
            checkpoints.set(count);
            if count >= 6 {
                Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"))
            } else {
                Ok(())
            }
        }
    )
    .is_err());
    assert!(checkpoints.get() >= 6);
}
