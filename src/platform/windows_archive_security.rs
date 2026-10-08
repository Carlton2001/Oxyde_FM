use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use windows::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_DIRECTORY, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
    FILE_SHARE_READ, FILE_SHARE_WRITE,
};

const MAX_PROVENANCE_BYTES: u64 = 16 * 1024;

pub(super) fn hold_directory(path: &Path) -> io::Result<File> {
    hold_directory_with(path, false)
}

pub(super) fn hold_operation_directory(path: &Path) -> io::Result<File> {
    hold_directory_with(path, true)
}

fn hold_directory_with(path: &Path, allow_reparse: bool) -> io::Result<File> {
    let file = if !allow_reparse {
        super::windows_no_reparse::open(
            path,
            true,
            windows::Win32::Foundation::GENERIC_READ.0,
            FILE_SHARE_READ.0,
            1,
        )?
    } else {
        OpenOptions::new()
            // Attribute-only access does not participate in Windows share checking.
            // Read access makes the omitted FILE_SHARE_DELETE block directory renames.
            .read(true)
            .share_mode(if allow_reparse {
                FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0
            } else {
                FILE_SHARE_READ.0
            })
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS.0 | FILE_FLAG_OPEN_REPARSE_POINT.0)
            .open(path)?
    };
    let metadata = file.metadata()?;
    let reparse = super::platform().file_attributes(&metadata, "").2;
    if metadata.file_attributes() & FILE_ATTRIBUTE_DIRECTORY.0 == 0 || (!allow_reparse && reparse) {
        return Err(io::Error::other(
            "Archive parent is not an ordinary directory",
        ));
    }
    if allow_reparse && reparse {
        // Deny renames and data writers on existing junctions; attribute-only
        // reparse mutation is still possible and requires atomic native lookup.
        // Ordinary operation parents allow write sharing: native file renames
        // open the destination directory for adding a child. This does not
        // prohibit installing a NEW reparse point on an ordinary directory.
        return OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ.0)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS.0 | FILE_FLAG_OPEN_REPARSE_POINT.0)
            .open(path);
    }
    Ok(file)
}

pub(super) fn ensure_directory(path: &Path) -> io::Result<File> {
    super::windows_no_reparse::open(
        path,
        true,
        windows::Win32::Foundation::GENERIC_READ.0,
        FILE_SHARE_READ.0,
        3,
    )
}
pub(super) fn create_file(path: &Path) -> io::Result<File> {
    super::windows_no_reparse::open(
        path,
        false,
        windows::Win32::Foundation::GENERIC_READ.0 | windows::Win32::Foundation::GENERIC_WRITE.0,
        FILE_SHARE_READ.0,
        2,
    )
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
    fn attribute_only_reparse_mutation_cannot_redirect_archive_outputs() {
        use std::os::windows::{io::AsRawHandle, process::CommandExt};
        use windows::Win32::Foundation::HANDLE;
        use windows::Win32::Storage::FileSystem::{FILE_SHARE_DELETE, FILE_WRITE_ATTRIBUTES};
        use windows::Win32::System::{
            Ioctl::{FSCTL_GET_REPARSE_POINT, FSCTL_SET_REPARSE_POINT},
            IO::DeviceIoControl,
        };
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("target");
        std::fs::create_dir(&target).unwrap();
        let template = root.path().join("template");
        let result = std::process::Command::new(super::super::platform().system_executable(crate::platform::SystemExecutable::PowerShell).unwrap())
            .args(["-NoProfile", "-NonInteractive", "-Command", "New-Item -ItemType Junction -Path $env:OXYDE_TEST_LINK -Value $env:OXYDE_TEST_TARGET -ErrorAction Stop | Out-Null"])
            .env("OXYDE_TEST_LINK", &template).env("OXYDE_TEST_TARGET", &target)
            .creation_flags(0x08000000).output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let read = OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS.0 | FILE_FLAG_OPEN_REPARSE_POINT.0)
            .open(&template)
            .unwrap();
        let mut data = vec![0u8; 16 * 1024];
        let mut size = 0;
        unsafe {
            DeviceIoControl(
                HANDLE(read.as_raw_handle()),
                FSCTL_GET_REPARSE_POINT,
                None,
                0,
                Some(data.as_mut_ptr().cast()),
                data.len() as u32,
                Some(&mut size),
                None,
            )
        }
        .unwrap();
        drop(read);
        let directory = root.path().join("held");
        std::fs::create_dir(&directory).unwrap();
        let held = hold_directory(&directory).unwrap();
        let writer = OpenOptions::new()
            .access_mode(FILE_WRITE_ATTRIBUTES.0)
            .share_mode(FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0 | FILE_SHARE_DELETE.0)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS.0 | FILE_FLAG_OPEN_REPARSE_POINT.0)
            .open(&directory);
        if let Ok(writer) = writer {
            let result = unsafe {
                DeviceIoControl(
                    HANDLE(writer.as_raw_handle()),
                    FSCTL_SET_REPARSE_POINT,
                    Some(data.as_ptr().cast()),
                    size,
                    None,
                    0,
                    Some(&mut size),
                    None,
                )
            };
            result.unwrap();
            assert!(create_file(&directory.join("output")).is_err());
            assert!(ensure_directory(&directory.join("subdirectory")).is_err());
            assert!(!target.join("output").exists());
            assert!(!target.join("subdirectory").exists());
        }
        drop(held);
        std::fs::remove_dir(template).unwrap();
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
