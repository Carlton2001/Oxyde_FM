use super::*;
use summaries::TreeSummaries;
mod summaries;

#[cfg(test)]
#[path = "conflicts/tests/entries.rs"]
mod tests;

pub(super) fn check_conflicts_real(
    paths: Vec<String>,
    target_dir: String,
) -> Result<Vec<ConflictEntry>, CommandError> {
    let mut conflicts = Vec::new();
    let mut summaries = TreeSummaries::default();
    let target_base = validate_path(&target_dir)?;

    let sources: Vec<_> = paths.iter().map(PathBuf::from).collect();
    let trash_entries = crate::platform::platform()
        .trash_transfer_entries(&sources)
        .map_err(CommandError::TrashError)?;
    for path_str in paths {
        let source_path = validate_path(&path_str)?;

        // Conflict check
        let display_path = trash_entries
            .iter()
            .find(|entry| entry.path == source_path)
            .map(|entry| entry.original_path.as_path())
            .unwrap_or(&source_path);
        if let Some(file_name) = display_path.file_name() {
            let target_path = target_base.join(file_name);
            if crate::systems::transfer_naming::same_entry(&source_path, &target_path) {
                continue;
            }
            if target_path.exists() {
                // Initial top-level conflict found
                add_conflict_recursive(&source_path, &target_path, &mut conflicts, &mut summaries)?;
            }
        }
    }

    Ok(conflicts)
}

fn add_conflict_recursive(
    source_path: &std::path::Path,
    target_path: &std::path::Path,
    conflicts: &mut Vec<ConflictEntry>,
    summaries: &mut TreeSummaries,
) -> Result<(), CommandError> {
    let mut source_entry = get_file_entry_from_path(source_path)?;
    let mut target_entry = get_file_entry_from_path(target_path)?;
    source_entry.name = target_entry.name.clone();

    if source_entry.is_dir {
        summaries.get(source_path).apply(&mut source_entry);
        summaries.get(target_path).apply(&mut target_entry);

        // Add the folder conflict itself
        conflicts.push(ConflictEntry {
            name: target_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
            source: source_entry,
            target: target_entry,
        });

        // Now recursively check children for conflicts
        for entry in std::fs::read_dir(source_path)? {
            let entry = entry?;
            let entry_path = entry.path();
            let entry_name = entry.file_name();
            let target_child_path = target_path.join(&entry_name);

            if target_child_path.exists() {
                add_conflict_recursive(&entry_path, &target_child_path, conflicts, summaries)?;
            }
        }
    } else {
        // Individual file conflict
        conflicts.push(ConflictEntry {
            name: target_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
            source: source_entry,
            target: target_entry,
        });
    }

    Ok(())
}
