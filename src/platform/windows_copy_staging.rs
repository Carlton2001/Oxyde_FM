//! Create and clean copy staging using retained directory identities.
use std::{
    ffi::OsStr,
    fs::File,
    io,
    os::windows::{fs::MetadataExt, io::AsRawHandle},
    path::Path,
};
use windows::Win32::{
    Foundation::{GENERIC_READ, GENERIC_WRITE, HANDLE},
    Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, DELETE, FILE_READ_ATTRIBUTES, FILE_SHARE_READ,
        FILE_SHARE_WRITE, FILE_WRITE_ATTRIBUTES, GetFileInformationByHandle,
    },
};

fn ordinary_directory(directory: &File) -> io::Result<()> {
    let metadata = directory.metadata()?;
    if !metadata.is_dir() || metadata.file_attributes() & 0x400 != 0 {
        return Err(io::Error::other(
            "Copy directory changed during the operation",
        ));
    }
    Ok(())
}

pub(super) fn create(parent: &File, path: &Path) -> io::Result<(File, File)> {
    ordinary_directory(parent)?;
    let leaf = path
        .file_name()
        .ok_or_else(|| io::Error::other("Staging path has no file name"))?;
    let directory = super::windows_no_reparse::create_relative(
        parent,
        leaf,
        true,
        GENERIC_READ.0 | DELETE.0,
        FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0,
        2, // FILE_ATTRIBUTE_HIDDEN
    )?;
    let pin = match super::windows_no_reparse::create_relative(
        &directory,
        OsStr::new(".pin"),
        false,
        GENERIC_READ.0 | GENERIC_WRITE.0 | DELETE.0,
        FILE_SHARE_READ.0,
        2,
    ) {
        Ok(pin) => pin,
        Err(error) => {
            let _ = super::windows_file_moves::remove(&directory);
            return Err(error);
        }
    };
    // The non-deletable pin keeps staging nonempty; a directory reparse point
    // cannot be installed while children remain. Staging likewise pins its parent.
    if let Err(error) = ordinary_directory(&directory).and_then(|_| ordinary_directory(parent)) {
        let _ = remove(&directory, pin, true);
        return Err(error);
    }
    Ok((directory, pin))
}

pub(super) fn create_output(parent: &File, path: &Path) -> io::Result<File> {
    ordinary_directory(parent)?;
    let leaf = path
        .file_name()
        .ok_or_else(|| io::Error::other("Archive output has no name"))?;
    super::windows_no_reparse::create_relative(
        parent,
        leaf,
        false,
        GENERIC_READ.0 | GENERIC_WRITE.0 | DELETE.0 | FILE_WRITE_ATTRIBUTES.0,
        FILE_SHARE_READ.0,
        0x80,
    )
}

pub(super) fn remove(directory: &File, pin: File, include_output: bool) -> io::Result<()> {
    let result = if !include_output {
        Ok(())
    } else {
        let output = super::windows_no_reparse::open_entry(
            directory,
            OsStr::new("output"),
            DELETE.0 | FILE_READ_ATTRIBUTES.0 | FILE_WRITE_ATTRIBUTES.0,
            FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0,
        );
        match output {
            Ok(output) => remove_output(&output),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    };
    // Drop the pin before deleting the directory, including failure paths.
    let unpin = super::windows_file_moves::remove(&pin);
    drop(pin);
    result?;
    unpin?;
    // Refuse unknown children instead of recursively deleting an altered tree.
    super::windows_file_moves::remove(directory)
}

pub(super) fn remove_output(output: &File) -> io::Result<()> {
    let metadata = output.metadata()?;
    let original = metadata.permissions();
    // Never change attributes on a substituted link or a directory.
    let clear_readonly =
        metadata.is_file() && metadata.file_attributes() & 0x400 == 0 && original.readonly();
    if clear_readonly {
        let mut information = BY_HANDLE_FILE_INFORMATION::default();
        unsafe { GetFileInformationByHandle(HANDLE(output.as_raw_handle()), &mut information) }
            .map_err(|error| {
                io::Error::from_raw_os_error((error.code().0 as u32 & 0xffff) as i32)
            })?;
        // Attributes are shared by hard links; do not alter another name.
        if information.nNumberOfLinks != 1 {
            return Err(io::Error::other(
                "Readonly copied output has multiple links",
            ));
        }
        let mut permissions = original.clone();
        permissions.set_readonly(false);
        output.set_permissions(permissions)?;
    }
    let result = super::windows_file_moves::remove(output);
    if let Err(error) = result {
        if clear_readonly {
            output.set_permissions(original).map_err(|restore| {
                io::Error::other(format!(
                    "{error}; cannot restore copy attributes: {restore}"
                ))
            })?;
        }
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::windows_reparse_tests::{Retarget, junction};
    use std::fs;

    #[test]
    fn refused_output_deletion_restores_readonly_on_the_same_handle() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("output");
        fs::write(&path, b"unchanged").unwrap();
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&path, permissions.clone()).unwrap();
        let parent = super::super::windows_file_moves::hold_parent(root.path()).unwrap();
        // Deliberately omit DELETE to exercise an OS refusal after attribute
        // clearing, without substituting the implementation under test.
        let file = super::super::windows_no_reparse::open_relative(
            &parent,
            OsStr::new("output"),
            false,
            GENERIC_READ.0 | FILE_WRITE_ATTRIBUTES.0,
            FILE_SHARE_READ.0,
            1,
        )
        .unwrap();
        let error = remove_output(&file).unwrap_err();
        let restored = file.metadata().unwrap().permissions().readonly();
        drop(file);
        permissions.set_readonly(false);
        fs::set_permissions(&path, permissions).unwrap();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert!(restored);
        assert_eq!(fs::read(&path).unwrap(), b"unchanged");
    }

    #[test]
    fn relative_creation_never_uses_a_retargeted_parent() {
        let root = tempfile::tempdir().unwrap();
        let selected = root.path().join("selected");
        let outside = root.path().join("outside");
        fs::create_dir(&selected).unwrap();
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("witness"), b"unchanged").unwrap();
        let template = root.path().join("template");
        junction(&template, &outside);
        let retarget = Retarget::new(&selected, &template);
        let parent = super::super::windows_file_moves::hold_parent(&selected).unwrap();
        retarget.apply();
        // Exercise the native creation itself, including a mutation after the
        // caller's metadata check. No child may appear in the junction target.
        let result = super::super::windows_no_reparse::create_relative(
            &parent,
            OsStr::new("staging"),
            true,
            GENERIC_READ.0 | DELETE.0,
            FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0,
            2,
        );
        assert_eq!(fs::read_dir(&outside).unwrap().count(), 1);
        assert_eq!(fs::read(outside.join("witness")).unwrap(), b"unchanged");
        if let Ok(directory) = result {
            super::super::windows_file_moves::remove(&directory).unwrap();
        }
        retarget.clear();
        assert_eq!(fs::read_dir(&selected).unwrap().count(), 0);
        drop((parent, retarget));
        fs::remove_dir(template).unwrap();
    }

    #[test]
    fn empty_staging_is_pinned_until_cleanup() {
        let root = tempfile::tempdir().unwrap();
        let selected = root.path().join("selected");
        let outside = root.path().join("outside");
        fs::create_dir(&selected).unwrap();
        fs::create_dir(&outside).unwrap();
        let template = root.path().join("template");
        junction(&template, &outside);
        let parent = super::super::windows_file_moves::hold_parent(&selected).unwrap();
        let path = selected.join("staging");
        let (directory, pin) = create(&parent, &path).unwrap();
        assert_ne!(directory.metadata().unwrap().file_attributes() & 2, 0);
        assert!(fs::remove_file(path.join(".pin")).is_err());
        assert!(fs::rename(path.join(".pin"), path.join("replaced-pin")).is_err());
        assert!(fs::rename(&path, selected.join("replaced-staging")).is_err());
        let retarget_stage = Retarget::new(&path, &template);
        let retarget_parent = Retarget::new(&selected, &template);
        assert!(retarget_stage.try_apply().is_err());
        assert!(retarget_parent.try_apply().is_err());
        drop((retarget_stage, retarget_parent));
        remove(&directory, pin, true).unwrap();
        drop(directory);
        assert!(!path.exists());
        assert_eq!(fs::read_dir(&outside).unwrap().count(), 0);
        drop(parent);
        fs::remove_dir(template).unwrap();
    }

    #[test]
    fn exclusive_creation_preserves_colliding_directories_and_junction_targets() {
        let root = tempfile::tempdir().unwrap();
        let parent = super::super::windows_file_moves::hold_parent(root.path()).unwrap();
        let existing = root.path().join("existing");
        fs::create_dir(&existing).unwrap();
        fs::write(existing.join("witness"), b"unchanged").unwrap();
        let link = root.path().join("link");
        junction(&link, &existing);
        assert!(create(&parent, &existing).is_err());
        assert!(create(&parent, &link).is_err());
        assert_eq!(fs::read_dir(&existing).unwrap().count(), 1);
        assert_eq!(fs::read(existing.join("witness")).unwrap(), b"unchanged");
        drop(parent);
        fs::remove_dir(link).unwrap();
    }

    #[test]
    fn cleanup_removes_a_substituted_output_link_without_traversing_it() {
        let root = tempfile::tempdir().unwrap();
        let parent = super::super::windows_file_moves::hold_parent(root.path()).unwrap();
        let path = root.path().join("staging");
        let (directory, pin) = create(&parent, &path).unwrap();
        let outside = root.path().join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("witness"), b"unchanged").unwrap();
        junction(&path.join("output"), &outside);
        remove(&directory, pin, true).unwrap();
        drop(directory);
        assert!(!path.exists());
        assert_eq!(fs::read(outside.join("witness")).unwrap(), b"unchanged");
        assert_eq!(fs::read_dir(&outside).unwrap().count(), 1);
    }

    #[test]
    fn cleanup_preserves_unknown_children() {
        let root = tempfile::tempdir().unwrap();
        let parent = super::super::windows_file_moves::hold_parent(root.path()).unwrap();
        let path = root.path().join("staging");
        let (directory, pin) = create(&parent, &path).unwrap();
        fs::write(path.join("foreign"), b"unchanged").unwrap();
        fs::write(path.join("output"), b"partial").unwrap();
        remove(&directory, pin, true).unwrap_err();
        drop(directory);
        assert_eq!(fs::read(path.join("foreign")).unwrap(), b"unchanged");
        assert!(!path.join("output").exists());
        assert!(!path.join(".pin").exists());
    }

    #[test]
    fn cleanup_does_not_change_attributes_of_a_readonly_hardlink_target() {
        let root = tempfile::tempdir().unwrap();
        let parent = super::super::windows_file_moves::hold_parent(root.path()).unwrap();
        let path = root.path().join("staging");
        let (directory, pin) = create(&parent, &path).unwrap();
        let witness = root.path().join("witness");
        fs::write(&witness, b"unchanged").unwrap();
        fs::hard_link(&witness, path.join("output")).unwrap();
        let mut permissions = fs::metadata(&witness).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&witness, permissions.clone()).unwrap();
        remove(&directory, pin, true).unwrap_err();
        drop(directory);
        assert!(fs::metadata(&witness).unwrap().permissions().readonly());
        assert_eq!(fs::read(&witness).unwrap(), b"unchanged");
        assert!(!path.join(".pin").exists());
        permissions.set_readonly(false);
        fs::set_permissions(&witness, permissions).unwrap();
    }
}
