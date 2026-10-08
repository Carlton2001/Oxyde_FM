use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use windows::Win32::Storage::FileSystem::{
    FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ, FILE_SHARE_WRITE,
};

const MAX_PROVENANCE_BYTES: u64 = 16 * 1024;

pub(super) fn hold_directory(path: &Path) -> io::Result<File> {
    let file = OpenOptions::new()
        // Attribute-only access does not participate in Windows share checking.
        // Read access makes the omitted FILE_SHARE_DELETE block directory renames.
        .read(true)
        .share_mode(FILE_SHARE_READ.0)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS.0 | FILE_FLAG_OPEN_REPARSE_POINT.0)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_dir() || super::platform().file_attributes(&metadata, "").2 {
        return Err(io::Error::other(
            "Archive parent is not an ordinary directory",
        ));
    }
    Ok(file)
}

pub(super) fn create_file(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .share_mode(FILE_SHARE_READ.0)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT.0)
        .open(path)
}

fn provenance_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(":Zone.Identifier");
    PathBuf::from(name)
}

pub(super) fn read_provenance(path: &Path) -> io::Result<Option<Vec<u8>>> {
    let file = match File::open(provenance_path(path)) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let mut data = Vec::new();
    file.take(MAX_PROVENANCE_BYTES + 1).read_to_end(&mut data)?;
    if data.len() as u64 > MAX_PROVENANCE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Internet provenance exceeds the size limit",
        ));
    }
    Ok(Some(data))
}

pub(super) fn write_provenance(path: &Path, data: &[u8]) -> io::Result<()> {
    if data.len() as u64 > MAX_PROVENANCE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Internet provenance exceeds the size limit",
        ));
    }
    // Keep the main file identity stable while writing its alternate stream.
    let held = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT.0)
        .open(path)?;
    let metadata = held.metadata()?;
    if !metadata.is_file() || super::platform().file_attributes(&metadata, "").2 {
        return Err(io::Error::other(
            "Cannot mark a non-ordinary archive output",
        ));
    }
    let mut stream = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(provenance_path(path))?;
    stream.write_all(data)?;
    stream.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn held_directories_block_writers_that_could_install_reparse_points() {
        use windows::Win32::Storage::FileSystem::FILE_SHARE_DELETE;
        let root = tempfile::tempdir().unwrap();
        let open_writer = || {
            OpenOptions::new()
                .write(true)
                .share_mode(FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0 | FILE_SHARE_DELETE.0)
                .custom_flags(FILE_FLAG_BACKUP_SEMANTICS.0)
                .open(root.path())
        };
        drop(open_writer().unwrap());
        let held = hold_directory(root.path()).unwrap();
        assert!(open_writer().is_err());
        drop(held);
        assert!(open_writer().is_ok());
    }

    #[test]
    fn archive_roots_and_parents_reject_junctions_without_touching_the_target() {
        use std::os::windows::process::CommandExt;
        let root = tempfile::tempdir().unwrap();
        let outside = root.path().join("outside");
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(outside.join("sentinel"), b"original").unwrap();
        let junction = root.path().join("junction");
        let command = PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
        let result = std::process::Command::new(command)
            .args(["-NoProfile", "-NonInteractive", "-Command",
                "New-Item -ItemType Junction -Path $env:OXYDE_JUNCTION_TEST_LINK -Value $env:OXYDE_JUNCTION_TEST_TARGET -ErrorAction Stop | Out-Null"])
            .env("OXYDE_JUNCTION_TEST_LINK", &junction)
            .env("OXYDE_JUNCTION_TEST_TARGET", &outside)
            .creation_flags(0x08000000)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{junction:?} -> {outside:?}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(hold_directory(&junction).is_err());
        assert!(crate::utils::archive_security::ExtractionRoot::new(&junction).is_err());
        let extraction = crate::utils::archive_security::ExtractionRoot::new(root.path()).unwrap();
        assert!(extraction.destination("junction/sentinel", false).is_err());
        assert_eq!(
            std::fs::read(outside.join("sentinel")).unwrap(),
            b"original"
        );
    }

    #[test]
    fn provenance_is_preserved_with_the_output_handle_open() {
        let root = tempfile::tempdir().unwrap();
        let output = root.path().join("file");
        let held = create_file(&output).unwrap();
        let data = b"[ZoneTransfer]\r\nZoneId=3\r\n";
        write_provenance(&output, data).unwrap();
        assert_eq!(
            read_provenance(&output).unwrap().as_deref(),
            Some(data.as_slice())
        );
        assert!(std::fs::rename(&output, root.path().join("renamed")).is_err());
        drop(held);
    }

    #[test]
    fn oversized_provenance_fails_without_being_silently_discarded() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("file");
        std::fs::write(&path, b"content").unwrap();
        std::fs::write(
            provenance_path(&path),
            vec![0; MAX_PROVENANCE_BYTES as usize + 1],
        )
        .unwrap();
        assert!(read_provenance(&path).is_err());
    }
}
