use super::{FileOperation, FileOperationManager};
use crate::models::ConflictAction;
use crate::platform::platform;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

impl FileOperationManager {
    pub(super) fn perform_archive_transfer(
        op: &Arc<Mutex<FileOperation>>,
        sources: Vec<PathBuf>,
        destination: Option<PathBuf>,
        is_move: bool,
        resolutions: Option<HashMap<String, ConflictAction>>,
    ) -> Result<(), String> {
        Self::perform_archive_transfer_with(op, sources, destination, is_move, resolutions, &mut || Ok(()))
    }

    pub(super) fn perform_archive_transfer_with(
        op: &Arc<Mutex<FileOperation>>,
        sources: Vec<PathBuf>,
        destination: Option<PathBuf>,
        is_move: bool,
        resolutions: Option<HashMap<String, ConflictAction>>,
        after_copy: &mut dyn FnMut() -> Result<(), String>,
    ) -> Result<(), String> {
        use crate::systems::archive_workspace::{
            lock_for_operation, mutation_locks, virtual_destination, virtual_source, writable,
            ArchiveWorkspaces,
        };

        Self::archive_checkpoint(op)?;
        let target = destination.ok_or("Missing destination")?;
        let target_archive = virtual_destination(&target)
            .map(|(archive, _)| std::fs::canonicalize(archive).map_err(|e| e.to_string()))
            .transpose()?;
        if target_archive.as_ref().is_some_and(|p| !writable(p))
            || (is_move
                && sources
                    .iter()
                    .filter_map(|p| virtual_source(p))
                    .any(|(archive, _)| !writable(&archive)))
        {
            return Err("This archive format is read-only".into());
        }
        if target_archive.as_ref().is_some_and(|archive| {
            sources
                .iter()
                .any(|source| std::fs::canonicalize(source).is_ok_and(|source| source == *archive))
        }) {
            return Err("Cannot put an archive inside itself".into());
        }
        let locks = mutation_locks(
            target_archive.iter().cloned().chain(
                sources
                    .iter()
                    .filter_map(|p| virtual_source(p).map(|(archive, _)| archive)),
            ),
        )?;
        let _guards = locks
            .iter()
            .map(|lock| lock_for_operation(lock, op))
            .collect::<Result<Vec<_>, _>>()?;
        let mut workspaces = ArchiveWorkspaces::default();
        let real_target = workspaces.resolve_for_operation(&target, true, op)?;
        let mut real_sources = Vec::new();
        for source in &sources {
            real_sources.push(workspaces.resolve_for_operation(source, false, op)?);
        }
        op.lock().unwrap().archive_path_mappings = workspaces
            .0
            .values()
            .map(|workspace| (workspace.root().to_path_buf(), workspace.archive.clone()))
            .collect();
        #[cfg(all(target_os = "windows", not(test)))]
        { op.lock().unwrap().archive_owned_roots = workspaces.0.values().map(|workspace| (workspace.root().to_path_buf(), workspace.cleanup_owner())).collect(); }
        Self::archive_checkpoint(op)?;
        let mapped_resolutions = resolutions
            .map(|resolutions| {
                resolutions
                    .into_iter()
                    .map(|(path, action)| {
                        workspaces
                            .resolve(Path::new(&path), false)
                            .map(|path| (path.to_string_lossy().into_owned(), action))
                    })
                    .collect::<Result<HashMap<_, _>, _>>()
            })
            .transpose()?;
        let deferred_moves = is_move.then(|| Arc::new(Mutex::new(
            crate::systems::archive_move_sources::ArchiveMoveSources::default(),
        )));
        let trash_entries = platform().trash_transfer_entries(&real_sources)?;
        Self::perform_copy_real(
            op,
            real_sources.clone(),
            Some(real_target.clone()),
            false,
            mapped_resolutions,
            &trash_entries,
            deferred_moves.clone(),
        )?;
        Self::archive_checkpoint(op)?;
        after_copy()?;
        let copied = op.lock().unwrap().copied_files.clone();
        let mut changed = std::collections::HashSet::new();
        if let Some(archive) = &target_archive {
            changed.insert(archive.clone());
        }
        // Stage removals only for files that were actually copied. Skipped conflicts stay put.
        if is_move {
            let mut moves = deferred_moves.as_ref().unwrap().lock().unwrap();
            moves.verify_all(&mut || Self::archive_checkpoint(op).map_err(std::io::Error::other))
                .map_err(|error| error.to_string())?;
            moves.remove_matching_observed(&|source| workspaces.display(source) != source,
                &mut || Self::archive_checkpoint(op).map_err(std::io::Error::other),
                &mut |source, _identity| {
                    #[cfg(all(target_os = "windows", not(test)))]
                    for workspace in workspaces.0.values() { if source.starts_with(workspace.root()) { workspace.forget_removed(source, _identity)?; } }
                    #[cfg(any(not(target_os = "windows"), test))]
                    let _ = source;
                    Ok(())
                })
                .map_err(|error| error.to_string())?;
            drop(moves);
            for (source, _) in &copied {
                for (archive, workspace) in &workspaces.0 {
                    if source.starts_with(workspace.root()) {
                        changed.insert(archive.clone());
                    }
                }
            }
            deferred_moves.as_ref().unwrap().lock().unwrap().remove_directories_matching_observed(
                &|source| workspaces.display(source) != source,
                &mut || Self::archive_checkpoint(op).map_err(std::io::Error::other),
                &mut |source, _, _identity| {
                    for (archive, workspace) in &workspaces.0 {
                        if source.starts_with(workspace.root()) {
                            #[cfg(all(target_os = "windows", not(test)))]
                            workspace.forget_removed(source, _identity)?;
                            changed.insert(archive.clone());
                        }
                    }
                    Ok(())
                },
            ).map_err(|error| error.to_string())?;
        }
        let retained_outputs = deferred_moves.as_ref().map(|moves| moves.lock().unwrap().retained_outputs())
            .transpose().map_err(|error| error.to_string())?.unwrap_or_default();
        // Prepare every replacement before any original is moved. Stable order
        // keeps the destination first, with rollback across the complete group.
        if let Some(archive) = &target_archive { changed.remove(archive); }
        let mut remaining: Vec<_> = changed.into_iter().collect();
        remaining.sort();
        let ordered: Vec<_> = target_archive.iter().cloned().chain(remaining).collect();
        crate::systems::archive_workspace::commit_group(&workspaces, &ordered, op, &retained_outputs)?;
        drop(retained_outputs);
        if is_move {
            // Sources have remained held since copy, including during reconstruction.
            deferred_moves.as_ref().unwrap().lock().unwrap().remove_matching(
                &|source| workspaces.display(source) == source,
                &mut || Self::archive_checkpoint(op).map_err(std::io::Error::other),
            ).map_err(|error| error.to_string())?;
            deferred_moves.as_ref().unwrap().lock().unwrap().remove_directories_matching(
                &|source| workspaces.display(source) == source,
                &mut || Self::archive_checkpoint(op).map_err(std::io::Error::other),
                &mut |_, _| {},
            ).map_err(|error| error.to_string())?;
        }
        // These effects are not filesystem Undo entries: their paths may belong to a temporary view.
        let mut locked = op.lock().unwrap();
        locked.created_files.clear();
        locked.created_dirs.clear();
        locked.overwritten_files.clear();
        locked.copied_files.clear();
        locked.copied_file_hashes.clear();
        locked.copied_file_identities.clear();
        locked.copied_directory_identities.clear();
        locked.created_links.clear();
        locked.moved_links.clear();
        locked.directory_attributes.clear();
        locked.renamed_dirs.clear();
        locked.moved_file_identities.clear();
        locked.moved_directory_identities.clear();
        Ok(())
    }

    pub(super) fn remove_archive_entries(
        op: &Arc<Mutex<FileOperation>>,
        archive: PathBuf,
        internal: Vec<String>,
    ) -> Result<(), String> {
        use crate::systems::archive_workspace::{
            lock_for_operation, mutation_locks, writable, ArchiveWorkspace,
        };
        let locks = mutation_locks([archive.clone()])?;
        let _lock = lock_for_operation(&locks[0], op)?;
        if !writable(&archive) {
            return Err("This archive format is read-only".into());
        }
        Self::archive_checkpoint(op)?;
        let workspace = ArchiveWorkspace::open_for_operation(archive, op)?;
        op.lock().unwrap().archive_path_mappings =
            vec![(workspace.root().to_path_buf(), workspace.archive.clone())];
        for path in internal {
            Self::archive_checkpoint(op)?;
            let path = workspace.path(&path)?;
            workspace.remove(&path, &mut || Self::archive_checkpoint(op).map_err(std::io::Error::other)).map_err(|e| e.to_string())?;
        }
        workspace.commit(op)
    }
}
