use super::*;
use std::io::Read;

#[test]
fn turbo_seven_can_pause_and_cancel_after_source_is_buffered_without_publication() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("input");
    let mut state = 97u32;
    let bytes: Vec<_> = (0..8 * 1024 * 1024)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state as u8
        })
        .collect();
    fs::write(&source, &bytes).unwrap();
    let output = root.path().join("cancelled.7z");
    let op = operation(FileOpType::Compress);
    op.lock().unwrap().compression_quality = "best".into();
    op.lock().unwrap().turbo_flag.store(true, Ordering::Relaxed);
    let (work, input, target) = (op.clone(), source.clone(), output.clone());
    let task = std::thread::spawn(move || {
        native(|| FileOperationManager::perform_compress(&work, vec![input], Some(target)))
    });
    let started = Instant::now();
    while op.lock().unwrap().processed_files < 1 {
        assert!(
            !task.is_finished(),
            "Encoder ended before its source was read"
        );
        assert!(started.elapsed() < Duration::from_secs(30));
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(op.lock().unwrap().processed_bytes, bytes.len() as u64);
    op.lock().unwrap().pause_flag.store(true, Ordering::Release);
    std::thread::sleep(Duration::from_millis(350));
    assert!(!output.exists());
    assert!(!task.is_finished());
    let cancelled = Instant::now();
    op.lock()
        .unwrap()
        .cancel_flag
        .store(true, Ordering::Release);
    assert!(task.join().unwrap().is_err());
    assert!(cancelled.elapsed() < Duration::from_secs(4));
    assert!(!output.exists());
    assert_eq!(fs::read(source).unwrap(), bytes);
}

#[test]
fn production_turbo_roundtrips_multiple_files_and_uses_solid_seven_blocks() {
    native(|| {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        fs::create_dir(&source).unwrap();
        let mut state = 37u32;
        let content: Vec<_> = (0..4 * 1024 * 1024)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                state as u8
            })
            .collect();
        for name in ["a.bin", "b.bin", "c.bin"] {
            fs::write(source.join(name), &content).unwrap();
        }
        fs::write(source.join("empty"), []).unwrap();
        fs::create_dir(source.join("empty-dir")).unwrap();
        for extension in ["zip", "7z", "tar", "tar.zst"] {
            let output = root.path().join(format!("output.{extension}"));
            let op = operation(FileOpType::Compress);
            op.lock().unwrap().compression_quality = "fast".into();
            op.lock().unwrap().turbo_flag.store(true, Ordering::Relaxed);
            FileOperationManager::perform_compress(&op, vec![source.clone()], Some(output.clone()))
                .unwrap();
            // Independent readers verify bytes and CRC, outside the worker decoder.
            if extension == "zip" {
                let mut zip = zip::ZipArchive::new(fs::File::open(&output).unwrap()).unwrap();
                for name in ["a.bin", "b.bin", "c.bin"] {
                    let mut bytes = Vec::new();
                    zip.by_name(&format!("source/{name}"))
                        .unwrap()
                        .read_to_end(&mut bytes)
                        .unwrap();
                    assert_eq!(bytes, content);
                }
            }
            if extension == "7z" {
                let mut reader =
                    sevenz_rust2::ArchiveReader::open(&output, sevenz_rust2::Password::empty())
                        .unwrap();
                assert!(
                    reader.archive().blocks.len() < 3,
                    "Files must share solid blocks"
                );
                let mut count = 0;
                reader
                    .for_each_entries(|entry, input| {
                        let mut bytes = Vec::new();
                        input.read_to_end(&mut bytes).unwrap();
                        if entry.name().ends_with(".bin") {
                            assert_eq!(bytes, content);
                            count += 1;
                        }
                        Ok(true)
                    })
                    .unwrap();
                assert_eq!(count, 3);
            }
            let destination = root.path().join(format!("decoded-{extension}"));
            FileOperationManager::perform_extract(
                &operation(FileOpType::Extract),
                vec![output],
                Some(destination.clone()),
            )
            .unwrap();
            for name in ["a.bin", "b.bin", "c.bin"] {
                assert_eq!(
                    fs::read(destination.join("source").join(name)).unwrap(),
                    content
                );
            }
            assert!(destination.join("source/empty-dir").is_dir());
            assert_eq!(
                fs::metadata(destination.join("source/empty"))
                    .unwrap()
                    .len(),
                0
            );
        }
    });
}
