//! Indexed selection and independent, bounded restoration of Recycle Bin entries.
use super::*;
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub struct TrashRestoreReport {
    pub restored: Vec<String>,
    pub failures: Vec<(String, String)>,
}

fn restore_selected(
    items: Vec<trash::TrashItem>,
    paths: Vec<String>,
    mut restore: impl FnMut(trash::TrashItem) -> Result<(), String>,
) -> TrashRestoreReport {
    let mut ids = HashMap::new();
    let mut originals: HashMap<String, Vec<usize>> = HashMap::new();
    for (index, item) in items.iter().enumerate() {
        ids.insert(normalize_restore_path(std::path::Path::new(&item.id)), index);
        originals.entry(normalize_restore_path(&item.original_path())).or_default().push(index);
    }
    let mut report = TrashRestoreReport::default();
    let mut seen = HashSet::new();
    for path in paths {
        let key = normalize_restore_path(std::path::Path::new(&path));
        if !seen.insert(key.clone()) { continue; }
        let selected = ids.get(&key).copied().or_else(|| {
            originals.get(&key).filter(|matches| matches.len() == 1).map(|matches| matches[0])
        });
        let Some(index) = selected else {
            let error = crate::models::error::TrashRestoreFailure {
                issue: if originals.contains_key(&key) { crate::models::error::TrashRestoreIssue::Ambiguous }
                    else { crate::models::error::TrashRestoreIssue::Unavailable },
                path: path.clone(), detail: "Selection does not identify exactly one trash item".into(),
            }.to_string();
            log::warn!(target: "oxyde_lib::trash_restore", "stage=selection id={path:?} error={error}");
            report.failures.push((path, error));
            continue;
        };
        let item = &items[index];
        // Aliases (ID and original path) must never restore the same object twice.
        let id = normalize_restore_path(std::path::Path::new(&item.id));
        if id != key && !seen.insert(id) { continue; }
        let original = item.original_path().to_string_lossy().into_owned();
        let started = std::time::Instant::now();
        match restore(item.clone()) {
            Ok(()) => { log::debug!(target: "oxyde_lib::trash_restore", "stage=item_completed id={:?} destination={original:?} wall_s={:.6}", item.id, started.elapsed().as_secs_f64()); report.restored.push(original); }
            Err(error) => {
                log::warn!(target: "oxyde_lib::trash_restore", "stage=item_failed id={:?} destination={original:?} wall_s={:.6} error={error}", item.id, started.elapsed().as_secs_f64());
                report.failures.push((original, error));
            }
        }
        let processed = report.restored.len() + report.failures.len();
        if processed % 256 == 0 {
            info!(target: "oxyde_lib::trash_restore", "stage=progress processed={processed} restored={} failed={}", report.restored.len(), report.failures.len());
        }
    }
    report
}

pub async fn restore_items_report(paths: Vec<String>) -> Result<TrashRestoreReport, CommandError> {
    let started = std::time::Instant::now();
    info!(target: "oxyde_lib::trash_restore", "stage=start requested={}", paths.len());
    let items = crate::platform::platform().list_trash_items().map_err(|error| {
        log::warn!(target: "oxyde_lib::trash_restore", "stage=enumeration error={error}");
        CommandError::TrashError(error)
    })?;
    info!(target: "oxyde_lib::trash_restore", "stage=enumerated available={} wall_s={:.6}", items.len(), started.elapsed().as_secs_f64());
    let items = items.iter().map(item_with_correct_restore_name).collect();
    info!(target: "oxyde_lib::trash_restore", "stage=names_resolved wall_s={:.6}", started.elapsed().as_secs_f64());
    let report = restore_selected(items, paths, |item| crate::platform::platform().restore_trash_items(vec![item]));
    info!(target: "oxyde_lib::trash_restore", "stage=end restored={} failed={} wall_s={:.6}", report.restored.len(), report.failures.len(), started.elapsed().as_secs_f64());
    Ok(report)
}

#[cfg(test)]
#[path = "tests/restore_batch.rs"]
mod tests;
