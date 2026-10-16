use super::*;

#[test]
fn five_modes_copy_bytes_and_only_hash_when_requested() {
    let temporary = tempfile::tempdir().unwrap();
    for bytes in [
        Vec::new(),
        (0..1_100_000).map(|i| (i % 251) as u8).collect(),
    ] {
        let source = temporary.path().join("source");
        std::fs::write(&source, &bytes).unwrap();
        let expected = blake3::hash(&bytes).to_hex().to_string();
        for mode in Mode::ALL {
            let destination = temporary.path().join(mode.name());
            let result = copy_one(&source, &destination, mode)
                .unwrap_or_else(|error| panic!("{mode:?}, {} bytes: {error}", bytes.len()));
            assert_eq!(std::fs::read(&destination).unwrap(), bytes);
            assert_eq!(result.bytes, bytes.len() as u64);
            if matches!(
                mode,
                Mode::NativeHash | Mode::StreamHash | Mode::NativeSourceHash
            ) {
                assert_eq!(result.digest.as_deref(), Some(expected.as_str()));
            } else {
                assert!(result.digest.is_none());
            }
            assert!(copy_one(&source, &destination, mode).is_err());
            assert_eq!(std::fs::read(&destination).unwrap(), bytes);
            std::fs::remove_file(destination).unwrap();
        }
    }
    assert_eq!(std::fs::read_dir(temporary.path()).unwrap().count(), 1);
}

#[test]
fn native_copy_and_source_hash_run_concurrently() {
    let barrier = std::sync::Barrier::new(2);
    let result = run_parallel(
        || {
            barrier.wait();
            Ok(())
        },
        || {
            barrier.wait();
            Ok("digest".into())
        },
    )
    .unwrap();
    assert_eq!(result.digest, "digest");
}

#[test]
fn copy_failure_waits_for_the_hash_thread() {
    let finished = std::sync::atomic::AtomicBool::new(false);
    let result = run_parallel(
        || Err(io::Error::other("copy failed")),
        || {
            finished.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok("digest".into())
        },
    );
    assert!(result.is_err());
    assert!(finished.load(std::sync::atomic::Ordering::SeqCst));
}

#[test]
fn hash_failure_or_panic_is_reported_after_copy() {
    for panic in [false, true] {
        let result = run_parallel(
            || Ok(()),
            || {
                if panic {
                    panic!("injected reader panic");
                }
                Err(io::Error::other("hash failed"))
            },
        );
        assert!(result.is_err());
    }
}
