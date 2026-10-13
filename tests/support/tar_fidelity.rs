use super::*;

fn forged_worker(root: &Path) -> PathBuf {
    use std::os::windows::process::CommandExt;
    let path = root.join("forged-tar-worker.exe");
    let status = std::process::Command::new("rustc")
        .args(["--edition=2021", "-O", "-C", "panic=abort"])
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support/forged_tar_worker.rs"))
        .arg("-o")
        .arg(&path)
        .creation_flags(0x08000000)
        .status()
        .unwrap();
    assert!(status.success());
    path
}

#[test]
fn protocol_valid_forged_tar_encode_and_decode_are_refused_before_publication() {
    let root = tempfile::tempdir().unwrap();
    let worker = forged_worker(root.path());
    let input = root.path().join("input");
    fs::write(&input, b"safe").unwrap();
    let output = root.path().join("encoded.tar");
    let error = with_worker_executable_for_tests(&worker, || {
        FileOperationManager::perform_compress(
            &operation(FileOpType::Compress),
            vec![input.clone()],
            Some(output.clone()),
        )
    })
    .unwrap_err();
    assert!(
        error.contains("Archive fidelity verification failed"),
        "{error}"
    );
    assert!(!output.exists());
    assert_eq!(fs::read(&input).unwrap(), b"safe");
    let source = root.path().join("source.tar");
    {
        let mut builder = tar::Builder::new(fs::File::create(&source).unwrap());
        let mut header = tar::Header::new_gnu();
        header.set_size(4);
        header.set_mode(0o644);
        header.set_mtime(123);
        header.set_cksum();
        builder
            .append_data(&mut header, "doc", &b"safe"[..])
            .unwrap();
        builder.finish().unwrap();
    }
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    let before = fs::read(&source).unwrap();
    let error = with_worker_executable_for_tests(&worker, || {
        FileOperationManager::perform_extract(
            &operation(FileOpType::Extract),
            vec![source.clone()],
            Some(destination.clone()),
        )
    })
    .unwrap_err();
    assert!(
        error.contains("Archive fidelity verification failed"),
        "{error}"
    );
    assert_eq!(fs::read(&source).unwrap(), before);
    assert_eq!(fs::read_dir(destination).unwrap().count(), 0);
}

#[test]
fn real_tar_worker_preserves_posix_pax_paths_and_long_encoder_names_under_independent_verification()
{
    native(|| {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("pax.tar");
        {
            let mut builder = tar::Builder::new(fs::File::create(&source).unwrap());
            builder
                .append_pax_extensions([("path", &b"docs/chosen"[..])])
                .unwrap();
            let mut header = tar::Header::new_ustar();
            header.set_size(4);
            header.set_mode(0o644);
            header.set_mtime(123);
            header.set_cksum();
            builder
                .append_data(&mut header, "ignored", &b"safe"[..])
                .unwrap();
            builder.finish().unwrap();
        }
        let destination = root.path().join("decoded");
        FileOperationManager::perform_extract(
            &operation(FileOpType::Extract),
            vec![source],
            Some(destination.clone()),
        )
        .unwrap();
        assert_eq!(fs::read(destination.join("docs/chosen")).unwrap(), b"safe");
        let long = root.path().join("a".repeat(150));
        fs::write(&long, b"long source").unwrap();
        let output = root.path().join("long.tar");
        FileOperationManager::perform_compress(
            &operation(FileOpType::Compress),
            vec![long.clone()],
            Some(output.clone()),
        )
        .unwrap();
        let mut archive = tar::Archive::new(fs::File::open(output).unwrap());
        let mut entries = archive.entries().unwrap();
        let mut member = entries.next().unwrap().unwrap();
        assert_eq!(member.path().unwrap().file_name(), long.file_name());
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut member, &mut bytes).unwrap();
        assert_eq!(bytes, b"long source");
        drop(member);
        assert!(entries.next().is_none());
    });
}
