#![cfg(target_os = "windows")]
mod commands { pub use oxyde_lib::commands::archive_handle_worker; }
mod platform { pub use oxyde_lib::platform::platform; }
mod models {
    pub use oxyde_lib::models::error;
    pub use oxyde_lib::models::*;
}
mod utils {
    pub use oxyde_lib::utils::{archive_listing_protocol, preview_protocol};
    pub use oxyde_lib::utils::image_preview_decode;
}
#[allow(dead_code)]
#[path = "../src/platform/windows_archive_worker.rs"]
mod windows_archive_worker;
#[allow(dead_code)]
#[path = "../src/platform/windows_preview_worker.rs"]
mod windows_preview_worker;
#[allow(dead_code)]
#[path = "../src/platform/windows_process_launch.rs"]
mod windows_process_launch;
#[allow(dead_code)]
#[path = "../src/platform/windows_worker_budget.rs"]
mod windows_worker_budget;
#[allow(dead_code)]
#[path = "../src/platform/windows_resource_pressure.rs"]
mod windows_resource_pressure;
#[allow(dead_code)]
#[path = "../src/platform/windows_worker_sandbox.rs"]
mod windows_worker_sandbox;
use models::error::{ArchiveLimitFailure, ArchiveLimitKind, ArchiveListingFailure};
use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};

fn listing(path: &Path, internal: &str) -> Result<Vec<models::FileEntry>, models::CommandError> {
    windows_archive_worker::run_in(
        Path::new(env!("CARGO_BIN_EXE_oxyde-archive-list-worker")),
        path,
        internal,
        Duration::from_secs(120),
    )
}
fn index(path: &Path) -> Result<Vec<models::FileEntry>, models::CommandError> {
    windows_archive_worker::index_in(
        Path::new(env!("CARGO_BIN_EXE_oxyde-archive-list-worker")),
        path,
        Duration::from_secs(120),
        &|| Ok(()),
    )
}
fn zip(path: &Path, unsafe_name: bool) {
    let mut writer = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    for name in [
        "root.txt",
        if unsafe_name {
            "../outside.txt"
        } else {
            "docs/école.txt"
        },
    ] {
        writer
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"safe").unwrap();
    }
    writer.finish().unwrap();
}
#[test]
fn actual_worker_lists_zip_tar_compressions_seven_zip_and_iso_without_writing_source_or_outputs() {
    let directory = tempfile::tempdir().unwrap();
    let archive = directory.path().join("école.zip");
    zip(&archive, false);
    let before = std::fs::read(&archive).unwrap();
    let root = listing(&archive, "").unwrap();
    assert_eq!(root.len(), 2);
    assert!(root
        .iter()
        .any(|entry| entry.name == "docs" && entry.is_dir));
    let nested = listing(&archive, "docs").unwrap();
    assert_eq!(nested.len(), 1);
    assert_eq!(
        nested[0].path,
        archive.join("docs").join("école.txt").to_string_lossy()
    );
    assert_eq!(nested[0].size, 4);
    let all = index(&archive).unwrap();
    assert_eq!(all.len(), 2);
    assert!(all.iter().any(|entry| entry.name == "école.txt"
        && Path::new(&entry.path) == archive.join("docs").join("école.txt")));
    assert_eq!(std::fs::read(&archive).unwrap(), before);
    let mut builder = tar::Builder::new(Vec::new());
    let mut header = tar::Header::new_gnu();
    header.set_size(4);
    header.set_mode(0o644);
    header.set_cksum();
    builder
        .append_data(&mut header, "docs/école.txt", &b"safe"[..])
        .unwrap();
    let tar = builder.into_inner().unwrap();
    let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gzip.write_all(&tar).unwrap();
    let mut xz = xz2::write::XzEncoder::new(Vec::new(), 1);
    xz.write_all(&tar).unwrap();
    let mut bz = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::default());
    bz.write_all(&tar).unwrap();
    let zst = zstd::stream::encode_all(&tar[..], 1).unwrap();
    for (extension, data) in [
        ("tar", tar),
        ("tar.gz", gzip.finish().unwrap()),
        ("tar.xz", xz.finish().unwrap()),
        ("tar.bz2", bz.finish().unwrap()),
        ("tar.zst", zst),
    ] {
        let path = directory.path().join(format!("école.{extension}"));
        std::fs::write(&path, &data).unwrap();
        let entry = listing(&path, "docs").unwrap().remove(0);
        assert_eq!(entry.name, "école.txt");
        assert_eq!(entry.size, 4);
        let all = index(&path).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].path, entry.path);
        assert_eq!(all[0].name, entry.name);
        assert_eq!(all[0].modified, entry.modified);
        assert_eq!(std::fs::read(&path).unwrap(), data);
    }
    let path = directory.path().join("école.7z");
    let mut writer = sevenz_rust2::ArchiveWriter::create(&path).unwrap();
    let mut entry = sevenz_rust2::ArchiveEntry::new();
    entry.name = "docs/école.txt".into();
    entry.has_stream = true;
    entry.size = 4;
    writer
        .push_archive_entry(entry, Some(std::io::Cursor::new(b"safe")))
        .unwrap();
    writer.finish().unwrap();
    assert_eq!(listing(&path, "docs").unwrap()[0].size, 4);
    assert_eq!(index(&path).unwrap()[0].name, "école.txt");
    let path = directory.path().join("image.iso");
    iso_fixture(&path);
    let root = listing(&path, "").unwrap();
    assert!(root
        .iter()
        .any(|entry| entry.name == "DOCS" && entry.is_dir));
    assert_eq!(listing(&path, "DOCS").unwrap()[0].name, "README.TXT");
    assert!(index(&path)
        .unwrap()
        .iter()
        .any(|entry| entry.name == "README.TXT"
            && Path::new(&entry.path) == path.join("DOCS").join("README.TXT")));
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 8);
    // Terminated workers release their read handles before the caller returns.
    for item in std::fs::read_dir(directory.path()).unwrap() {
        std::fs::remove_file(item.unwrap().path()).unwrap();
    }
}

#[test]
fn actual_worker_refuses_unsafe_names_corruption_and_invalid_requests_without_side_effects() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("unsafe.zip");
    zip(&path, true);
    assert!(listing(&path, "").is_err());
    assert!(index(&path).is_err());
    assert!(!directory.path().join("outside.txt").exists());
    assert!(listing(&path, "../outside").is_err());
    assert!(listing(&path, "C:\\outside").is_err());
    assert!(listing(&directory.path().join("absent.zip"), "").is_err());
    std::fs::write(&path, b"not an archive").unwrap();
    assert!(listing(&path, "").is_err());
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
}
fn powershell(script: &str) -> std::process::Command {
    let path = oxyde_lib::platform::platform()
        .system_executable(oxyde_lib::platform::SystemExecutable::PowerShell)
        .unwrap();
    let mut command = std::process::Command::new(path);
    command.args(["-NoProfile", "-NonInteractive", "-Command", script]);
    command
}
#[test]
fn supervisor_times_out_bounds_stdout_and_refuses_failed_or_malformed_children_without_local_fallback(
) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("valid.zip");
    zip(&path, false);
    let started = Instant::now();
    let error = windows_archive_worker::supervise(
        powershell(
            "$g=[Console]::OpenStandardInput().ReadByte(); [Threading.Thread]::Sleep(10000)",
        ),
        &path,
        "",
        Duration::from_secs(2),
    )
    .unwrap_err();
    let limit = ArchiveLimitFailure::from_message(&error.to_string()).unwrap();
    assert_eq!(limit.kind, ArchiveLimitKind::Seconds);
    assert_eq!(limit.limit, 2);
    assert!(started.elapsed() < Duration::from_secs(5));
    let script="$g=[Console]::OpenStandardInput().ReadByte(); $o=[Console]::OpenStandardOutput(); $b=New-Object byte[] 65536; for($i=0;$i -lt 600;$i++){$o.Write($b,0,$b.Length)}";
    let error =
        windows_archive_worker::supervise(powershell(script), &path, "", Duration::from_secs(10))
            .unwrap_err();
    assert!(matches!(
        ArchiveListingFailure::from_message(&error.to_string()),
        Some(ArchiveListingFailure::MetadataLimit)
    ));
    for script in ["$g=[Console]::OpenStandardInput().ReadByte(); exit 9", "$g=[Console]::OpenStandardInput().ReadByte(); [Console]::OpenStandardOutput().WriteByte(0)"] {
        let error=windows_archive_worker::supervise(powershell(script),&path,"",Duration::from_secs(10)).unwrap_err();
        assert!(matches!(ArchiveListingFailure::from_message(&error.to_string()),Some(ArchiveListingFailure::WorkerFailed)),"{error}");
    }
    assert_eq!(listing(&path, "").unwrap().len(), 2);
}

#[test]
fn index_supervisor_cancellation_stops_stalled_child_and_never_launches_when_already_cancelled() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("valid with spaces.zip");
    zip(&path, false);
    let stopped = || {
        Err(std::io::Error::new(
            std::io::ErrorKind::Interrupted,
            "cancelled",
        ))
    };
    let error = windows_archive_worker::index_in(
        Path::new("C:/nonexistent-test-worker.exe"),
        &path,
        Duration::from_secs(120),
        &stopped,
    )
    .unwrap_err();
    assert!(error.to_string().contains("cancelled"));
    assert!(ArchiveListingFailure::from_message(&error.to_string()).is_none());
    let started = Instant::now();
    let checkpoint = || {
        if started.elapsed() > Duration::from_millis(200) {
            stopped()
        } else {
            Ok(())
        }
    };
    let error = windows_archive_worker::supervise_index(
        powershell(
            "$g=[Console]::OpenStandardInput().ReadByte(); [Threading.Thread]::Sleep(10000)",
        ),
        &path,
        Duration::from_secs(120),
        &checkpoint,
    )
    .unwrap_err();
    assert!(started.elapsed() < Duration::from_secs(4));
    assert!(error.to_string().contains("cancelled"));
    assert!(ArchiveListingFailure::from_message(&error.to_string()).is_none());
    for script in ["$g=[Console]::OpenStandardInput().ReadByte(); exit 9", "$g=[Console]::OpenStandardInput().ReadByte(); [Console]::OpenStandardOutput().WriteByte(0)"] {
        let error=windows_archive_worker::supervise_index(powershell(script),&path,Duration::from_secs(10),&||Ok(())).unwrap_err();
        assert!(matches!(ArchiveListingFailure::from_message(&error.to_string()),Some(ArchiveListingFailure::WorkerFailed)));
    }
    assert_eq!(index(&path).unwrap().len(), 2);
    std::fs::remove_file(&path).unwrap();
}

#[path = "support/iso.rs"]
mod iso;
use iso::iso_fixture;
