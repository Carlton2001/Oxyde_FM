use super::*;

/// A UI snapshot of per-drive settings; it never determines the operation type.
#[derive(Default)]
pub(crate) struct DeletePolicy {
    revision: u64,
    drives: Vec<(String, bool)>,
}

impl DeletePolicy {
    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    #[cfg(target_os = "windows")]
    pub(crate) fn invalidate(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    pub(crate) fn replace(&mut self, revision: u64, drives: Vec<(String, bool)>) -> bool {
        if revision != self.revision || self.drives == drives {
            return false;
        }
        self.drives = drives;
        true
    }

    #[cfg(target_os = "windows")]
    pub(crate) fn saved(&mut self, configs: &[TrashSystemConfig]) {
        self.invalidate();
        for config in configs {
            if let Some((_, permanent)) = self.drives.iter_mut().find(|(root, _)| {
                crate::ui::components::sidebar::sidebar_paths_equal(root, &config.drive_path)
            }) {
                *permanent = config.nuke_on_delete;
            } else {
                self.drives.push((config.drive_path.clone(), config.nuke_on_delete));
            }
        }
    }

    pub(crate) fn for_path(&self, path: &str) -> bool {
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

pub(crate) fn read_drive_policy(drives: &[DriveInfo]) -> Vec<(String, bool)> {
    drives.iter().map(|drive| {
        let permanent = oxyde_lib::platform::platform()
            .recycle_bin_config(drive.path.clone())
            .is_ok_and(|config| config.nuke_on_delete);
        (drive.path.clone(), permanent)
    }).collect()
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
pub(super) fn selection_deletes_permanently(paths: &[String]) -> bool {
    let mut roots = std::collections::HashMap::new();
    !paths.is_empty() && paths.iter().all(|path| {
        let Some(root) = std::path::Path::new(path).ancestors().last() else { return false; };
        let root = root.to_string_lossy().into_owned();
        *roots.entry(root.clone()).or_insert_with(|| {
            oxyde_lib::platform::platform().recycle_bin_config(root)
                .is_ok_and(|config| config.nuke_on_delete)
        })
    })
}

#[cfg(all(test, target_os = "windows"))]
#[path = "deletion_policy/tests/policy.rs"]
mod tests;
