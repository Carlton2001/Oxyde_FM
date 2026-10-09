//! Transfer naming is advisory; publication still uses exclusive native helpers.
use super::path_parents::HeldParents;
use crate::platform::platform;
use std::{
    collections::HashSet,
    ffi::OsString,
    io,
    path::{Path, PathBuf},
};

pub(crate) fn same_entry(source: &Path, target: &Path) -> bool {
    let compare = || -> io::Result<bool> {
        let source_parent = HeldParents::new(source)?;
        let target_parent = HeldParents::new(target)?;
        let source = platform().track_source_item(source_parent.directory(), source)?;
        let target = platform().track_source_item(target_parent.directory(), target)?;
        Ok(platform().file_identity(&source)? == platform().file_identity(&target)?)
    };
    compare().unwrap_or(false)
}

pub(super) fn copy_name(
    source: &Path,
    directory: &Path,
    is_directory: bool,
    suffix: &str,
    reserved: &mut HashSet<PathBuf>,
) -> io::Result<PathBuf> {
    if suffix.is_empty()
        || suffix
            .chars()
            .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid copy name suffix",
        ));
    }
    let stem = if is_directory {
        source.file_name()
    } else {
        source.file_stem()
    }
    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Missing copy source name"))?;
    let extension = (!is_directory).then(|| source.extension()).flatten();
    for index in 1..=10000 {
        let mut name = OsString::from(stem);
        name.push(format!(" - {suffix}"));
        if index > 1 {
            name.push(format!(" ({index})"));
        }
        if let Some(extension) = extension {
            name.push(".");
            name.push(extension);
        }
        let target = directory.join(name);
        if reserved.contains(&target) {
            continue;
        }
        match std::fs::symlink_metadata(&target) {
            Ok(_) => continue,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                reserved.insert(target.clone());
                return Ok(target);
            }
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "No available copy name",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn copy_names_preserve_extensions_and_never_select_an_existing_item() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("École.txt");
        std::fs::write(&source, "original").unwrap();
        std::fs::write(root.path().join("École - Copie.txt"), "witness").unwrap();
        let mut reserved = HashSet::new();
        assert_eq!(
            copy_name(&source, root.path(), false, "Copie", &mut reserved).unwrap(),
            root.path().join("École - Copie (2).txt")
        );
        assert_eq!(
            copy_name(&source, root.path(), false, "Copie", &mut reserved).unwrap(),
            root.path().join("École - Copie (3).txt")
        );
        assert_eq!(
            copy_name(
                &root.path().join("folder.ext"),
                root.path(),
                true,
                "Copy",
                &mut reserved
            )
            .unwrap(),
            root.path().join("folder.ext - Copy")
        );
        assert_eq!(std::fs::read(source).unwrap(), b"original");
        assert_eq!(
            std::fs::read(root.path().join("École - Copie.txt")).unwrap(),
            b"witness"
        );
    }
    #[test]
    fn comparison_recognizes_aliases_without_conflating_equal_contents() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("file.txt");
        std::fs::write(&source, "same").unwrap();
        let other = root.path().join("other.txt");
        std::fs::write(&other, "same").unwrap();
        assert!(same_entry(&source, &root.path().join("./file.txt")));
        assert!(!same_entry(&source, &other));
    }
}
