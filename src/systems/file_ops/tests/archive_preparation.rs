use super::*;
use crate::utils::archive_limits::{ArchiveBudget, ArchiveLimits};
use std::io::Write;

#[test]
fn late_cancellation_preserves_publication_and_recovery_status() {
    let mut op = FileOperation::new(FileOpType::Extract, vec![], None, None);
    op.cancel_flag.store(true, Ordering::Relaxed);
    FileOperationManager::finish_status(&mut op, OpStatus::Completed);
    assert_eq!(op.status, OpStatus::Cancelled);
    op.archive_publication_finished = true;
    FileOperationManager::finish_status(&mut op, OpStatus::Completed);
    assert_eq!(op.status, OpStatus::Completed);
    let recovery = OpStatus::Error(crate::models::error::ArchiveRecoveryFailure { completed: false, locations: vec!["C:/published.zip".into()], omitted: 0, cause: "cancelled".into() }.to_string());
    FileOperationManager::finish_status(&mut op, recovery.clone());
    assert_eq!(op.status, recovery);
    FileOperationManager::finish_status(&mut op, OpStatus::Error("cancelled before publication".into()));
    assert_eq!(op.status, OpStatus::Cancelled);
}

#[cfg(target_os = "windows")]
#[test]
fn compression_refuses_source_substitution_before_encoding_for_every_format() {
    for extension in ["zip", "7z", "tar", "zst"] {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source.txt");
        let saved = root.path().join("saved.txt");
        let output = root.path().join("output");
        std::fs::create_dir(&output).unwrap();
        std::fs::write(&source, b"same bytes").unwrap();
        let archive = output.join(format!("archive.{extension}"));
        let error = FileOperationManager::perform_compress_prepared(&operation(), vec![source.clone()], Some(archive.clone()), &mut || {
            std::fs::rename(&source, &saved)?;
            std::fs::write(&source, b"same bytes")
        }).unwrap_err();
        assert_eq!(crate::models::error::SourceSelectionFailure::from_message(&error), Some(true));
        assert!(!archive.exists());
        assert!(std::fs::read_dir(&output).unwrap().next().is_none(), "compression left staging after error: {extension}");
        assert_eq!(std::fs::read(&source).unwrap(), b"same bytes");
        assert_eq!(std::fs::read(&saved).unwrap(), b"same bytes");
    }
}

#[cfg(target_os = "windows")]
#[test]
fn compression_retains_source_and_publication_parents_after_junction_retargeting() {
    use crate::platform::windows_reparse_tests::{junction, Retarget};
    for extension in ["zip", "7z", "tar", "zst"] {
        let root = tempfile::tempdir().unwrap();
        for folder in ["chosen", "outside-source", "output", "outside-output"] { std::fs::create_dir(root.path().join(folder)).unwrap(); }
        std::fs::write(root.path().join("chosen/source.txt"), b"selected content").unwrap();
        std::fs::write(root.path().join("outside-source/source.txt"), b"source witness").unwrap();
        std::fs::write(root.path().join("outside-output/witness"), b"destination witness").unwrap();
        let source_link = root.path().join("source-link");
        let source_template = root.path().join("source-template");
        let output_link = root.path().join("output-link");
        let output_template = root.path().join("output-template");
        junction(&source_link, &root.path().join("chosen"));
        junction(&source_template, &root.path().join("outside-source"));
        junction(&output_link, &root.path().join("output"));
        junction(&output_template, &root.path().join("outside-output"));
        let source_retarget = Retarget::new(&source_link, &source_template);
        let output_retarget = Retarget::new(&output_link, &output_template);
        let archive = output_link.join(format!("archive.{extension}"));
        let op = Arc::new(Mutex::new(FileOperation::new(FileOpType::Compress, vec![], Some(archive.clone()), None)));
        FileOperationManager::perform_compress_prepared(&op, vec![source_link.join("source.txt")], Some(archive.clone()), &mut || {
            source_retarget.apply(); output_retarget.apply(); Ok(())
        }).unwrap();
        let produced = root.path().join("output").join(format!("archive.{extension}"));
        assert!(produced.is_file());
        assert!(!root.path().join("outside-output").join(format!("archive.{extension}")).exists());
        let workspace = crate::systems::archive_workspace::ArchiveWorkspace::open(produced.clone()).unwrap();
        assert_eq!(std::fs::read(workspace.root().join("source.txt")).unwrap(), b"selected content");
        assert_eq!(std::fs::read(root.path().join("outside-source/source.txt")).unwrap(), b"source witness");
        assert_eq!(std::fs::read(root.path().join("outside-output/witness")).unwrap(), b"destination witness");
        assert_eq!(std::fs::canonicalize(op.lock().unwrap().destination.as_ref().unwrap()).unwrap(), std::fs::canonicalize(produced).unwrap());
        assert_eq!(std::fs::read_dir(root.path().join("output")).unwrap().count(), 1);
        drop(workspace);
        drop((source_retarget, output_retarget));
        for link in [source_link, source_template, output_link, output_template] { std::fs::remove_dir(link).unwrap(); }
    }
}

#[cfg(target_os = "windows")]
#[test]
fn compression_preserves_archive_arriving_after_planning_and_uses_current_source_metadata() {
    for extension in ["zip", "7z", "tar", "zst"] {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("écriture.txt");
        let output = root.path().join("output");
        std::fs::create_dir(&output).unwrap();
        std::fs::write(&source, b"old").unwrap();
        let archive = output.join(format!("archive.{extension}"));
        let op = Arc::new(Mutex::new(FileOperation::new(FileOpType::Compress, vec![], Some(archive.clone()), None)));
        FileOperationManager::perform_compress_prepared(&op, vec![source.clone()], Some(archive.clone()), &mut || {
            std::fs::write(&archive, b"new destination witness")?;
            std::fs::write(&source, b"edited contents longer than the planned length")
        }).unwrap();
        assert_eq!(std::fs::read(&archive).unwrap(), b"new destination witness");
        let produced = op.lock().unwrap().destination.clone().unwrap();
        let workspace = crate::systems::archive_workspace::ArchiveWorkspace::open(produced).unwrap();
        assert_eq!(std::fs::read(workspace.root().join("écriture.txt")).unwrap(), b"edited contents longer than the planned length");
        assert_eq!(std::fs::read_dir(&output).unwrap().count(), 2);
    }
}

#[test]
fn extraction_failure_cleans_output_for_all_formats() {
    use super::*;
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("file.txt");
    std::fs::write(&source, b"bounded test content").unwrap();
    for extension in ["zip", "7z", "tar", "zst", "iso"] {
        let archive = temp.path().join(format!("limited.{extension}"));
        if extension == "iso" {
            crate::commands::archive::test_iso_image(&archive);
        } else {
            FileOperationManager::perform_compress(
                &crate::systems::archive_workspace::mutation_operation("fast"),
                vec![source.clone()],
                Some(archive.clone()),
            )
            .unwrap();
        }
        for cancelled in [false, true] {
            let output = temp.path().join(format!("output-{extension}-{cancelled}"));
            let op = Arc::new(Mutex::new(FileOperation::new(
                FileOpType::Extract,
                vec![archive.clone()],
                Some(output.clone()),
                None,
            )));
            op.lock()
                .unwrap()
                .cancel_flag
                .store(cancelled, Ordering::Relaxed);
            let limits = ArchiveLimits {
                max_bytes: 2,
                ..ArchiveLimits::EXTRACTION
            };
            assert!(FileOperationManager::perform_extract_with_limits(
                &op,
                vec![archive.clone()],
                Some(output.clone()),
                limits
            )
            .is_err());
            assert!(!output.exists(), "partial output kept for {extension}");
            assert!(archive.is_file());
        }
    }
}

#[cfg(target_os = "windows")]
#[test]
fn extraction_retains_source_and_provenance_through_parent_retargeting() {
    use crate::platform::windows_reparse_tests::{junction, Retarget};
    for extension in ["zip", "7z", "tar", "zst", "iso"] {
        let temp = tempfile::tempdir().unwrap();
        let chosen = temp.path().join("chosen");
        let outside = temp.path().join("outside");
        std::fs::create_dir(&chosen).unwrap();
        std::fs::create_dir(&outside).unwrap();
        let archive = chosen.join(format!("source.{extension}"));
        let foreign = outside.join(format!("source.{extension}"));
        if extension == "iso" {
            crate::commands::archive::test_iso_image(&archive);
            crate::commands::archive::test_iso_image(&foreign);
        } else {
            for (folder, content) in [(&chosen, &b"safe"[..]), (&outside, &b"evil"[..])] {
                let source = folder.join("selected.txt");
                std::fs::write(&source, content).unwrap();
                FileOperationManager::perform_compress(&operation(), vec![source], Some(folder.join(format!("source.{extension}")))).unwrap();
            }
        }
        let original = std::fs::read(&archive).unwrap();
        let foreign_bytes = std::fs::read(&foreign).unwrap();
        let provenance = b"[ZoneTransfer]\r\nZoneId=3\r\n";
        platform().write_file_provenance(&archive, provenance).unwrap();
        let link = temp.path().join("link");
        let template = temp.path().join("template");
        junction(&link, &chosen);
        junction(&template, &outside);
        let retarget = Retarget::new(&link, &template);
        let target = temp.path().join("output");
        let op = operation();
        let called = std::cell::Cell::new(false);
        FileOperationManager::perform_extract_prepared(&op, vec![link.join(format!("source.{extension}"))], Some(target.clone()), ArchiveLimits::EXTRACTION, &mut || {
            called.set(true);
            assert!(std::fs::File::options().write(true).open(&archive).is_err());
            assert!(std::fs::rename(&archive, chosen.join("moved.archive")).is_err());
            retarget.apply();
            Ok(())
        }).unwrap();
        assert!(called.get());
        let output = target.join(if extension == "iso" { "ROOT.TXT" } else { "selected.txt" });
        let expected = if extension == "iso" { &b"root"[..] } else { &b"safe"[..] };
        assert_eq!(std::fs::read(&output).unwrap(), expected);
        assert_eq!(platform().read_file_provenance(&output).unwrap().as_deref(), Some(&provenance[..]));
        assert_eq!(std::fs::read(&archive).unwrap(), original);
        assert_eq!(std::fs::read(&foreign).unwrap(), foreign_bytes);
        assert!(std::fs::File::options().write(true).open(&archive).is_ok());
        drop(retarget);
        for path in [link, template] { std::fs::remove_dir(path).unwrap(); }
    }
}

#[test]
fn extraction_source_errors_precede_destination_creation() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("output");
    assert!(FileOperationManager::perform_extract(&operation(), vec![temp.path().join("missing.zip")], Some(target.clone())).is_err());
    assert!(!target.exists());
    assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 0);
}

#[test]
fn zip_full_extraction_validates_late_entries_before_writing_early_files() {
    let temp = tempfile::tempdir().unwrap();
    let archive = temp.path().join("unsafe.zip");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&archive).unwrap());
    for name in ["early.txt", "file:stream"] {
        zip.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(b"safe").unwrap();
    }
    zip.finish().unwrap();
    let target = tempfile::tempdir().unwrap();
    let file = platform().open_archive_source(&archive, false).unwrap();
    assert!(crate::commands::archive::extract_open_archive_with_budget(&archive, file, &target.path().to_string_lossy(), &|| Ok(()), &ArchiveBudget::new(ArchiveLimits::EXTRACTION)).is_err());
    assert_eq!(std::fs::read_dir(target.path()).unwrap().count(), 0);
    let output = temp.path().join("output");
    assert!(FileOperationManager::perform_extract(&operation(), vec![archive], Some(output.clone())).is_err());
    assert!(!output.exists());
}

#[test]
fn zip_operations_reject_windows_escape_and_alternate_stream_names() {
    use super::*;
    for name in ["a/C:victim.txt", "a/../C:victim.txt", "file.txt:extra"] {
        let temp = tempfile::tempdir().unwrap();
        let archive = temp.path().join("unsafe.zip");
        let mut writer = zip::ZipWriter::new(std::fs::File::create(&archive).unwrap());
        writer
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"marker").unwrap();
        writer.finish().unwrap();
        let output = temp.path().join("output");
        let op = Arc::new(Mutex::new(FileOperation::new(
            FileOpType::Extract,
            vec![archive.clone()],
            Some(output.clone()),
            None,
        )));
        assert!(FileOperationManager::perform_extract(
            &op,
            vec![archive],
            Some(output.clone())
        )
        .is_err());
        assert!(!output.exists());
    }
}

#[cfg(target_os = "windows")]
#[test]
fn zip_operations_mark_outputs_before_publication() {
    use super::*;
    let temp = tempfile::tempdir().unwrap();
    let archive = temp.path().join("marked.zip");
    let mut writer = zip::ZipWriter::new(std::fs::File::create(&archive).unwrap());
    writer
        .start_file("file.txt", zip::write::SimpleFileOptions::default())
        .unwrap();
    writer.write_all(b"marker").unwrap();
    writer.finish().unwrap();
    let data = b"[ZoneTransfer]\r\nZoneId=3\r\n";
    platform().write_file_provenance(&archive, data).unwrap();
    let output = temp.path().join("output");
    let op = Arc::new(Mutex::new(FileOperation::new(
        FileOpType::Extract,
        vec![archive.clone()],
        Some(output.clone()),
        None,
    )));
    FileOperationManager::perform_extract(&op, vec![archive], Some(output.clone())).unwrap();
    assert_eq!(
        platform()
            .read_file_provenance(&output.join("file.txt"))
            .unwrap()
            .as_deref(),
        Some(data.as_slice())
    );
}
