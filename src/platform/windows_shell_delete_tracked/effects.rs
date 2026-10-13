//! Bounded descendant receipts. An absent path alone is never a delete receipt.
use crate::models::transaction::{HistoryDeletedItem, HistoryFileIdentity};
use crate::systems::path_parents::HeldParents;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use windows::core::{Ref, HRESULT};
use windows::Win32::Foundation::S_OK;
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::{
    IShellItem, COPYENGINE_S_DONT_PROCESS_CHILDREN, SIGDN_FILESYSPATH,
};

const MAX_ITEMS: usize = 65_536;
const MAX_NAMES: usize = 8 * 1024 * 1024;

fn key(path: &Path) -> String {
    path.to_string_lossy()
        .replace('/', "\\")
        .trim_start_matches("\\\\?\\")
        .to_owned()
}
pub(super) fn path(item: Ref<IShellItem>) -> Result<PathBuf, String> {
    let raw = unsafe {
        item.ok()
            .map_err(|e| e.to_string())?
            .GetDisplayName(SIGDN_FILESYSPATH)
    }
    .map_err(|e| e.to_string())?;
    let value = unsafe { raw.to_string() }
        .map(PathBuf::from)
        .map_err(|e| e.to_string());
    unsafe {
        CoTaskMemFree(Some(raw.0.cast()));
    }
    value
}
#[derive(Default)]
pub(super) struct Effects {
    pending: HashMap<String, (PathBuf, Option<HistoryFileIdentity>)>,
    completed: HashMap<String, (HistoryDeletedItem, Option<PathBuf>, bool)>,
    names: usize,
}
impl Effects {
    pub(super) fn pre(&mut self, root: &Path, item: Ref<IShellItem>) -> Result<(), String> {
        let path = path(item)?;
        let name = key(&path);
        if !name
            .to_lowercase()
            .starts_with(&format!("{}\\", key(root).to_lowercase()))
        {
            return Ok(());
        }
        if self.pending.contains_key(&name) || self.completed.contains_key(&name) {
            return Ok(());
        }
        if self.pending.len() + self.completed.len() >= MAX_ITEMS
            || name.len() > MAX_NAMES.saturating_sub(self.names)
        {
            return Err("Deletion receipt budget exceeded; remaining items were stopped".into());
        }
        let proof = (|| -> std::io::Result<_> {
            let parents = HeldParents::new(&path)?;
            let file = super::super::platform()
                .track_source_item(parents.directory(), &parents.resolved_path(&path)?)?;
            HistoryFileIdentity::from_file(&file)
        })()
        .map_err(|e| {
            format!(
                "Cannot capture deletion receipt for {}: {e}",
                path.display()
            )
        })?;
        self.names += name.len();
        self.pending.insert(name, (path, Some(proof)));
        Ok(())
    }
    pub(super) fn post(
        &mut self,
        item: Ref<IShellItem>,
        result: HRESULT,
        payload: Ref<IShellItem>,
    ) -> Result<(), String> {
        let name = key(&path(item)?);
        let Some((path, proof)) = self.pending.remove(&name) else {
            return Ok(());
        };
        if matches!(result, S_OK | COPYENGINE_S_DONT_PROCESS_CHILDREN) {
            let recycled = payload.ok().is_ok();
            let payload = if recycled {
                self::path(payload).map(Some)
            } else {
                Ok(None)
            };
            self.completed.insert(
                name,
                (
                    HistoryDeletedItem::new(path.to_string_lossy().into_owned(), proof),
                    payload.as_ref().ok().cloned().flatten(),
                    recycled,
                ),
            );
            // A metadata failure must never discard a confirmed deletion.
            payload?;
        } else {
            self.names -= name.len();
        }
        Ok(())
    }
    pub(super) fn take(
        &mut self,
        root_completed: bool,
        root_payload: Option<&Path>,
    ) -> Vec<HistoryDeletedItem> {
        // A completed directory covers its completed children. Keep only roots
        // so restoration cannot collide with its own nested receipts.
        let mut completed = self
            .completed
            .drain()
            .filter(|(_, (_, payload, recycled))| {
                if !root_completed {
                    return true;
                }
                if !recycled {
                    return false;
                }
                let Some(payload) = payload else {
                    return true;
                };
                root_payload.is_none_or(|root| {
                    let root = key(root);
                    let child = key(payload);
                    child != root && !child.starts_with(&format!("{root}\\"))
                })
            })
            .collect::<Vec<_>>();
        completed.sort_by(|a, b| a.0.len().cmp(&b.0.len()).then_with(|| a.0.cmp(&b.0)));
        let mut accepted: HashMap<String, (HistoryDeletedItem, Option<PathBuf>)> = HashMap::new();
        let mut roots = Vec::new();
        for (path, (entry, payload, recycled)) in completed {
            let mut ancestor = path.as_str();
            let mut covered = false;
            let mut owner = None;
            while let Some((parent, _)) = ancestor.rsplit_once('\\') {
                if let Some((parent_entry, parent_payload)) = accepted.get(parent) {
                    // Original ancestry alone cannot subsume a child recycled
                    // into its own separate payload during a partial operation.
                    covered = !recycled
                        || payload.as_ref().zip(parent_payload.as_ref()).is_some_and(
                            |(child, root)| key(child).starts_with(&format!("{}\\", key(root))),
                        );
                    if !covered {
                        owner = Some(parent_entry.clone());
                    }
                    break;
                }
                ancestor = parent;
            }
            if !covered {
                accepted.insert(path, (entry.clone(), payload));
                if let Some(owner) = owner {
                    owner.remember_partial(vec![entry]);
                } else {
                    roots.push(entry);
                }
            }
        }
        roots
    }
}
