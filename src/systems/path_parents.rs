//! Hold both the requested namespace and resolved directory parents on Windows.
use crate::platform::platform;
use std::fs::File;
use std::io;
use std::path::Path;

pub(crate) struct HeldParents {
    _handles: Vec<File>,
}
impl HeldParents {
    pub fn new(file: &Path) -> io::Result<Self> {
        let absolute = std::path::absolute(file)?;
        let parent = absolute
            .parent()
            .ok_or_else(|| io::Error::other("Path has no parent"))?;
        let mut handles = Vec::new();
        // Existing junctions are usable. Hold their names and resolved parents.
        // This does not prevent attribute-only reparse mutation: these guards
        // provide parent rename protection, not complete path confinement.
        for ancestor in parent.ancestors().collect::<Vec<_>>().into_iter().rev() {
            handles.push(platform().hold_operation_directory(ancestor)?);
        }
        let resolved = std::fs::canonicalize(parent)?;
        for ancestor in resolved.ancestors().collect::<Vec<_>>().into_iter().rev() {
            handles.push(platform().hold_operation_directory(ancestor)?);
        }
        if std::fs::canonicalize(parent)? != resolved {
            return Err(io::Error::other("File parent changed during the operation"));
        }
        Ok(Self { _handles: handles })
    }
}

pub(crate) fn remove(
    path: &Path,
    checkpoint: &mut dyn FnMut() -> io::Result<()>,
) -> io::Result<()> {
    let _parents = HeldParents::new(path)?;
    checkpoint()?;
    if path.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    }
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;
    #[test]
    fn deletion_keeps_parent_fixed_until_the_selected_file_is_removed() {
        let root = tempfile::tempdir().unwrap();
        let parent = root.path().join("parent");
        std::fs::create_dir(&parent).unwrap();
        let selected = parent.join("selected");
        std::fs::write(&selected, b"selected").unwrap();
        let witness = root.path().join("witness");
        std::fs::write(&witness, b"outside").unwrap();
        remove(&selected, &mut || {
            assert!(std::fs::rename(&parent, root.path().join("replaced")).is_err());
            Ok(())
        })
        .unwrap();
        assert!(!selected.exists());
        assert_eq!(std::fs::read(witness).unwrap(), b"outside");
        std::fs::rename(&parent, root.path().join("replaced")).unwrap();
    }

    #[test]
    fn existing_junctions_remain_usable_and_both_namespaces_are_held() {
        use std::os::windows::process::CommandExt;
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("target");
        std::fs::create_dir(&target).unwrap();
        let link = root.path().join("junction");
        let output = std::process::Command::new(platform().system_executable(crate::platform::SystemExecutable::PowerShell).unwrap())
            .args(["-NoProfile", "-NonInteractive", "-Command", "New-Item -ItemType Junction -Path $env:OXYDE_TEST_LINK -Value $env:OXYDE_TEST_TARGET -ErrorAction Stop | Out-Null"])
            .env("OXYDE_TEST_LINK", &link).env("OXYDE_TEST_TARGET", &target)
            .creation_flags(0x08000000).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let held = HeldParents::new(&link.join("file")).unwrap();
        use std::os::windows::fs::OpenOptionsExt;
        let writer = std::fs::OpenOptions::new()
            .write(true)
            .share_mode(7)
            .custom_flags(
                windows::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS.0
                    | windows::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT.0,
            )
            .open(&link);
        assert!(
            writer.is_err(),
            "held junction must reject a retargeting writer"
        );

        assert!(std::fs::rename(&link, root.path().join("new-link")).is_err());
        assert!(std::fs::rename(&target, root.path().join("new-target")).is_err());
        std::fs::write(link.join("file"), b"allowed").unwrap();
        drop(held);
        std::fs::remove_dir(link).unwrap();
    }
}
