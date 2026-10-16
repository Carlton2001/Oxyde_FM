use crate::models::{CommandError, TrashEntry};
use std::path::PathBuf;

use super::original_trash_path;

/// List all items in the Windows Recycle Bin
pub fn list_trash() -> Result<Vec<TrashEntry>, CommandError> {
    let mut entries = list_trash_unsorted()?;
    entries.sort_by_cached_key(|entry| (!entry.is_dir, entry.name.to_lowercase()));
    Ok(entries)
}

/// The tab applies its configured sort; avoid sorting the same listing twice.
pub(crate) fn list_trash_unsorted() -> Result<Vec<TrashEntry>, CommandError> {
    let started = std::time::Instant::now();
    match crate::platform::platform().list_trash_entries() {
        Ok(Some(entries)) => {
            log::info!(target: "oxyde_lib::trash_listing",
                "mode=native_display items={} total_s={:.6}", entries.len(), started.elapsed().as_secs_f64());
            return Ok(entries);
        }
        Ok(None) => {}
        Err(error) => log::warn!(target: "oxyde_lib::trash_listing",
            "Native display listing failed, falling back to Shell properties: {error}"),
    }
    let trash_items =
        crate::platform::platform().list_trash_items().map_err(CommandError::TrashError)?;

    let enumerated = started.elapsed();
    let entries: Vec<TrashEntry> = trash_items
        .into_iter()
        .map(|item| {
            let original_path = original_trash_path(&item);
            let mut name = original_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| original_path.to_string_lossy().to_string());

            // On Windows, Shell API sometimes returns names without .lnk even if original_path had it.
            // Check the real trash file extension to be sure.
            let trash_path = PathBuf::from(&item.id);
            if let Some(ext) = trash_path.extension() {
                let ext_str = ext.to_string_lossy().to_lowercase();
                if (ext_str == "lnk" || ext_str == "url")
                    && !name.to_lowercase().ends_with(&format!(".{}", ext_str))
                {
                    name.push_str(&format!(".{}", ext_str));
                }
            }
            let deleted_time = (item.time_deleted.max(0) as u64) * 1000;

            // Reuse one snapshot for type, size and modification time.
            let metadata = std::fs::metadata(&trash_path).ok();
            let is_dir = metadata.as_ref().is_some_and(|metadata| metadata.is_dir());
            let size = if is_dir {
                0
            } else {
                metadata.as_ref().map_or(0, |metadata| metadata.len())
            };
            let modified = metadata
                .as_ref()
                .and_then(|metadata| metadata.modified().ok())
                .map(|t| {
                    t.duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64
                })
                .unwrap_or(deleted_time);

            TrashEntry {
                name,
                // Use the internal trash path (item.id) as unique identifier
                path: trash_path.to_string_lossy().to_string(),
                original_path: original_path.to_string_lossy().to_string(),
                is_dir,
                size,
                deleted_time,
                modified,
            }
        })
        .collect();

    log::info!(target: "oxyde_lib::trash_listing",
        "items={} enumeration_s={:.6} enrichment_s={:.6} total_s={:.6}",
        entries.len(), enumerated.as_secs_f64(),
        started.elapsed().saturating_sub(enumerated).as_secs_f64(),
        started.elapsed().as_secs_f64());
    Ok(entries)
}
