use super::*;

#[test]
fn crashing_cache_child() {
    let Some(base) = std::env::var_os("OXYDE_CACHE_CRASH_FIXTURE") else {
        return;
    };
    let base = PathBuf::from(base);
    let receipt = base.join("crash-receipt");
    native(|| {
        oxyde_lib::commands::archive::with_archive_cache_base_for_tests(&base, || {
            oxyde_lib::utils::archive_mutation_test::with_hook_for_tests(
                move |stage, _, data| {
                    if stage == oxyde_lib::utils::archive_mutation_test::Stage::CacheWritten {
                        fs::write(
                            &receipt,
                            data.parent().unwrap().to_string_lossy().as_bytes(),
                        )?;
                        std::process::exit(45);
                    }
                    Ok(())
                },
                || {
                    oxyde_lib::commands::archive::prepare_archive_file_for_open(
                        &base.join("input.zip/document"),
                    )
                },
            )
        })
    })
    .unwrap();
    panic!("Cache crash checkpoint was not reached");
}

#[test]
fn native_cache_recovers_a_durable_entry_after_the_preparing_process_exits_without_cleanup() {
    use std::os::windows::process::CommandExt;
    let base = tempfile::tempdir().unwrap();
    zip(
        &base.path().join("input.zip"),
        "document",
        b"owned payload",
        false,
    );
    let mut child = std::process::Command::new(std::env::current_exe().unwrap());
    child
        .args([
            "--exact",
            "cache_crash_recovery::crashing_cache_child",
            "--nocapture",
        ])
        .env("OXYDE_CACHE_CRASH_FIXTURE", base.path())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .creation_flags(0x08000000);
    assert_eq!(child.status().unwrap().code(), Some(45));
    let old = PathBuf::from(fs::read_to_string(base.path().join("crash-receipt")).unwrap());
    assert!(old.starts_with(fs::canonicalize(base.path()).unwrap()));
    assert_eq!(
        fs::read(old.join("data/document")).unwrap(),
        b"owned payload"
    );
    // Exercise the actual sixty-second launch grace, without weakening it for
    // production or rewriting a protected ownership record in the fixture.
    std::thread::sleep(Duration::from_secs(61));
    let opened = native(|| {
        oxyde_lib::commands::archive::with_archive_cache_base_for_tests(base.path(), || {
            oxyde_lib::commands::archive::prepare_archive_file_for_open(
                &base.path().join("input.zip/document"),
            )
        })
    })
    .unwrap();
    assert_eq!(fs::read(opened).unwrap(), b"owned payload");
    assert!(!old.exists());
}
