#![cfg(target_os = "windows")]
mod models {
    pub use oxyde_lib::models::error;
    pub use oxyde_lib::models::*;
}
mod utils {
    pub use oxyde_lib::utils::{archive_listing_protocol, preview_protocol};
}
#[allow(dead_code)]
#[path = "../src/platform/windows_archive_worker.rs"]
mod windows_archive_worker;
#[allow(dead_code)]
#[path = "../src/platform/windows_preview_worker.rs"]
mod windows_preview_worker;
use models::error::{ArchiveLimitFailure, ArchiveLimitKind, ArchiveListingFailure};
use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};

fn listing(path: &Path, internal: &str) -> Result<Vec<models::FileEntry>, models::CommandError> {
    windows_archive_worker::run_in(
        Path::new(env!("CARGO_BIN_EXE_oxyde")),
        path,
        internal,
        Duration::from_secs(120),
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
    let path = directory.path().join("image.iso");
    iso_fixture(&path);
    let root = listing(&path, "").unwrap();
    assert!(root
        .iter()
        .any(|entry| entry.name == "DOCS" && entry.is_dir));
    assert_eq!(listing(&path, "DOCS").unwrap()[0].name, "README.TXT");
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

fn iso_fixture(path: &Path) {
    fn record(name: &[u8], sector: u32, length: u32, directory: bool) -> Vec<u8> {
        let mut bytes = vec![0; (33 + name.len() + 1) & !1];
        bytes[0] = bytes.len() as u8;
        bytes[2..6].copy_from_slice(&sector.to_le_bytes());
        bytes[6..10].copy_from_slice(&sector.to_be_bytes());
        bytes[10..14].copy_from_slice(&length.to_le_bytes());
        bytes[14..18].copy_from_slice(&length.to_be_bytes());
        bytes[18..25].copy_from_slice(&[126, 10, 4, 0, 0, 0, 0]);
        bytes[25] = if directory { 2 } else { 0 };
        bytes[28..30].copy_from_slice(&1u16.to_le_bytes());
        bytes[30..32].copy_from_slice(&1u16.to_be_bytes());
        bytes[32] = name.len() as u8;
        bytes[33..33 + name.len()].copy_from_slice(name);
        bytes
    }
    let mut bytes = vec![0u8; 32 * 2048];
    let primary = &mut bytes[16 * 2048..17 * 2048];
    primary[0..7].copy_from_slice(b"\x01CD001\x01");
    primary[8..72].fill(b' ');
    primary[40..45].copy_from_slice(b"OXYDE");
    primary[80..84].copy_from_slice(&32u32.to_le_bytes());
    primary[84..88].copy_from_slice(&32u32.to_be_bytes());
    for offset in [120, 124] {
        primary[offset..offset + 2].copy_from_slice(&1u16.to_le_bytes());
        primary[offset + 2..offset + 4].copy_from_slice(&1u16.to_be_bytes());
    }
    primary[128..130].copy_from_slice(&2048u16.to_le_bytes());
    primary[130..132].copy_from_slice(&2048u16.to_be_bytes());
    primary[132..136].copy_from_slice(&22u32.to_le_bytes());
    primary[136..140].copy_from_slice(&22u32.to_be_bytes());
    primary[140..144].copy_from_slice(&18u32.to_le_bytes());
    primary[148..152].copy_from_slice(&19u32.to_be_bytes());
    primary[156..190].copy_from_slice(&record(&[0], 20, 2048, true));
    primary[190..813].fill(b' ');
    for offset in [813, 830, 847, 864] {
        primary[offset..offset + 16].fill(b'0');
    }
    primary[881] = 1;
    bytes[17 * 2048..17 * 2048 + 7].copy_from_slice(b"\xffCD001\x01");
    for (sector, big) in [(18, false), (19, true)] {
        let table = &mut bytes[sector * 2048..(sector + 1) * 2048];
        for (offset, name, extent) in [(0usize, &[0][..], 20u32), (10usize, &b"DOCS"[..], 21u32)] {
            table[offset] = name.len() as u8;
            table[offset + 2..offset + 6].copy_from_slice(&if big {
                extent.to_be_bytes()
            } else {
                extent.to_le_bytes()
            });
            table[offset + 6..offset + 8].copy_from_slice(&if big {
                1u16.to_be_bytes()
            } else {
                1u16.to_le_bytes()
            });
            table[offset + 8..offset + 8 + name.len()].copy_from_slice(name);
        }
    }
    for (sector, records) in [
        (
            20,
            vec![
                record(&[0], 20, 2048, true),
                record(&[1], 20, 2048, true),
                record(b"DOCS", 21, 2048, true),
                record(b"ROOT.TXT;1", 22, 4, false),
            ],
        ),
        (
            21,
            vec![
                record(&[0], 21, 2048, true),
                record(&[1], 20, 2048, true),
                record(b"README.TXT;1", 23, 11, false),
            ],
        ),
    ] {
        let mut offset = sector * 2048;
        for record in records {
            bytes[offset..offset + record.len()].copy_from_slice(&record);
            offset += record.len();
        }
    }
    bytes[22 * 2048..22 * 2048 + 4].copy_from_slice(b"root");
    bytes[23 * 2048..23 * 2048 + 11].copy_from_slice(b"ISO content");
    std::fs::write(path, bytes).unwrap();
}
