use super::*;

/// A UI snapshot of per-drive deletion, for labels and icons; it never
/// determines the operation type. A drive deletes permanently by setting or
/// because it has no Recycle Bin at all (mapped share, SUBST, removable drive).
#[derive(Default)]
pub(crate) struct DeletePolicy {
    revision: u64,
    drives: Vec<(String, bool)>,
    without_bin: Vec<String>,
}

impl DeletePolicy {
    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    #[cfg(target_os = "windows")]
    pub(crate) fn invalidate(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    pub(crate) fn replace(&mut self, revision: u64, survey: DriveDeletion) -> bool {
        let (drives, without_bin) = survey;
        if revision != self.revision || (self.drives == drives && self.without_bin == without_bin) {
            return false;
        }
        self.drives = drives;
        self.without_bin = without_bin;
        true
    }

    #[cfg(target_os = "windows")]
    pub(crate) fn saved(&mut self, configs: &[TrashSystemConfig]) {
        self.invalidate();
        for config in configs {
            // A setting never gives a Recycle Bin to a drive without one.
            let without_bin = self.without_bin.iter()
                .any(|root| crate::ui::paths::paths_match(root, &config.drive_path));
            if let Some((_, permanent)) = self.drives.iter_mut().find(|(root, _)| {
                crate::ui::paths::paths_match(root, &config.drive_path)
            }) {
                *permanent = config.nuke_on_delete || without_bin;
            } else {
                self.drives.push((config.drive_path.clone(), config.nuke_on_delete));
            }
        }
    }

    pub(crate) fn for_path(&self, path: &str) -> bool {
        // Windows has no Recycle Bin on a share named by its UNC path.
        #[cfg(target_os = "windows")]
        if (path.starts_with(r"\\") && !path.starts_with(r"\\?\")) || path.starts_with(r"\\?\UNC\") {
            return true;
        }
        // A verbatim DOS path refers to the same drive as its ordinary form.
        let path = path.strip_prefix(r"\\?\")
            .filter(|rest| rest.as_bytes().get(1) == Some(&b':'))
            .unwrap_or(path);
        self.drives.iter()
            .filter(|(root, _)| super::super::drive_watchers::path_on_drive(path, root))
            .max_by_key(|(root, _)| root.len())
            .is_some_and(|(_, permanent)| *permanent)
    }
}

/// Each drive with whether it deletes permanently, and the drives without a Recycle Bin.
pub(crate) type DriveDeletion = (Vec<(String, bool)>, Vec<String>);

/// Blocking; call on a background worker.
pub(crate) fn read_drive_policy(drives: &[DriveInfo]) -> DriveDeletion {
    let platform = oxyde_lib::platform::platform();
    let without_bin: Vec<String> = drives.iter()
        .filter(|drive| !platform.has_recycle_bin(std::path::Path::new(&drive.path)))
        .map(|drive| drive.path.clone())
        .collect();
    let drives = drives.iter().map(|drive| {
        let permanent = without_bin.contains(&drive.path)
            || platform.recycle_bin_config(drive.path.clone()).is_ok_and(|config| config.nuke_on_delete);
        (drive.path.clone(), permanent)
    }).collect();
    (drives, without_bin)
}

impl OxydeApp {
    pub(crate) fn pane_delete_is_permanent(&self, pane_id: &str) -> bool {
        let Some(pane) = self.state.find_pane(pane_id) else { return false; };
        let tab = pane.active_tab();
        if tab.selected_paths.is_empty() {
            self.trash_delete_policy.for_path(&tab.path)
        } else {
            tab.selected_paths.iter().all(|path| self.trash_delete_policy.for_path(path))
        }
    }
}

/// Refresh the warning from Windows immediately before showing confirmation.
/// Keep the captured selection and Shift intent independent of the current pane.
/// True when no selected item can be recycled: its volume deletes permanently
/// by setting, or has no Recycle Bin at all (share, SUBST, removable drive).
pub(super) fn selection_deletes_permanently(paths: &[String]) -> bool {
    let mut roots = std::collections::HashMap::new();
    !paths.is_empty() && paths.iter().all(|path| {
        // Windows decides per drive; elsewhere the item's own path says which mount it is on.
        let root = if cfg!(windows) {
            let Some(root) = std::path::Path::new(path).ancestors().last() else { return false; };
            root.to_string_lossy().into_owned()
        } else {
            path.clone()
        };
        *roots.entry(root.clone()).or_insert_with(|| {
            let platform = oxyde_lib::platform::platform();
            !platform.has_recycle_bin(std::path::Path::new(&root))
                || platform.recycle_bin_config(root).is_ok_and(|config| config.nuke_on_delete)
        })
    })
}

#[cfg(all(test, target_os = "windows"))]
#[path = "deletion_policy/tests/policy.rs"]
mod tests;
