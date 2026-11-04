//! Background reads of the Viewer's tree. Each read publishes only into the tree
//! of the file it was started for.
use super::{archive_levels, ArchiveInfo, Contents, Failure, Folder, Level, Listing, Toggle, Tree, READ_PER_FOLDER};
use crate::ui::app::viewer::{content::Content, Viewer};
use gpui::*;
use oxyde_lib::models::FileEntry;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
};

pub(super) fn row_height() -> Pixels {
    crate::ui::scale::rem_pixels(super::render::ROW_HEIGHT)
}

/// The archive holding the tree, for a navigable archive or a folder inside one.
/// A nested archive is a plain member: it has no tree.
fn archive_of(file: &FileEntry) -> Option<PathBuf> {
    if file.is_navigable_archive() {
        return Some(PathBuf::from(&file.path));
    }
    file.archive_path.as_ref().filter(|_| file.is_dir).map(|archive| archive.to_path_buf())
}

/// Whether the Viewer shows `file` as a tree.
pub(in crate::ui::app::viewer) fn has_tree(file: &FileEntry) -> bool {
    (file.is_dir && !oxyde_lib::platform::platform().is_remote_uri(&file.path)) || archive_of(file).is_some()
}

struct Read {
    folder: Folder,
    contents: Vec<(String, Contents)>,
}

fn read_local(path: &str, listing: Listing, cancelled: &AtomicBool) -> Result<Read, Failure> {
    let (entries, complete) = oxyde_lib::commands::io::list_local_dir_bounded(
        path, listing.show_hidden, listing.show_system, READ_PER_FOLDER,
    )
    .map_err(|error| {
        log::warn!("Viewer could not list {path}: {error}");
        Failure::Unreadable
    })?;
    let mut entries = entries;
    crate::ui::workspace::resolve_native_kinds(&mut entries);
    let folder = Folder::new(entries, complete, listing.sort);
    let mut contents = Vec::new();
    for entry in folder.entries.iter().filter(|entry| entry.is_dir) {
        if cancelled.load(Ordering::Relaxed) {
            break;
        }
        let (entries, subfolders) = oxyde_lib::commands::io::visible_contents(&entry.path, listing.show_hidden, listing.show_system);
        contents.push((entry.path.clone(), Contents { entries, subfolders }));
    }
    Ok(Read { folder, contents })
}

struct Indexed {
    levels: HashMap<String, Folder>,
    info: ArchiveInfo,
}

fn read_archive(archive: PathBuf, listing: Listing, cancelled: &AtomicBool) -> Result<Indexed, Failure> {
    let entries = oxyde_lib::commands::archive::index_archive_for_preview(&archive, cancelled).map_err(|error| {
        if oxyde_lib::models::error::ArchivePasswordFailure::from_message(&error.to_string()).is_some() {
            Failure::Locked
        } else {
            log::warn!("Viewer could not index {}: {error}", archive.display());
            Failure::Unreadable
        }
    })?;
    let mut entries = entries;
    crate::ui::workspace::resolve_native_kinds(&mut entries);
    let extracted = entries.iter().filter(|entry| !entry.is_dir).map(|entry| entry.size).sum();
    let levels = archive_levels(&archive, entries)
        .into_iter()
        .map(|(path, entries)| (path, Folder::new(entries, true, listing.sort)))
        .collect();
    let packed = (!oxyde_lib::utils::archive_volumes::is_rar_volume_set(&archive))
        .then(|| std::fs::metadata(&archive).ok().map(|metadata| metadata.len()))
        .flatten();
    let password = oxyde_lib::commands::archive::verify_archive_password(&archive).is_err_and(|error| {
        oxyde_lib::models::error::ArchivePasswordFailure::from_message(&error.to_string()).is_some()
    });
    Ok(Indexed { levels, info: ArchiveInfo { extracted, packed, password } })
}

impl Viewer {
    fn tree_mut(&mut self, generation: u64) -> Option<&mut Tree> {
        match &mut self.content {
            Content::Tree(tree) if self.generation == generation => Some(tree),
            _ => None,
        }
    }

    pub(in crate::ui::app::viewer) fn load_tree(&mut self, file: &FileEntry, cx: &mut Context<Self>) {
        let tree = Tree::new(file.path.clone(), self.source.listing);
        let cancelled = tree.cancelled.clone();
        let listing = tree.listing;
        let root = tree.root.clone();
        self.replace_content(Content::Tree(tree), cx);
        let generation = self.generation;
        let Some(archive) = archive_of(file) else {
            self.read_tree_level(root, cx);
            return;
        };
        let work = self.load_work(move || Ok::<_, String>(read_archive(archive, listing, &cancelled)));
        cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                let Some(tree) = this.tree_mut(generation) else { return };
                match result {
                    Ok((Ok(indexed), _completion)) => {
                        for (path, folder) in &indexed.levels {
                            tree.contents.insert(path.clone(), Contents { entries: folder.total > 0, subfolders: folder.folders > 0 });
                        }
                        tree.levels = indexed.levels.into_iter().map(|(path, folder)| (path, Level::Ready(folder))).collect();
                        if !tree.levels.contains_key(&tree.root) {
                            tree.levels.insert(tree.root.clone(), Level::Failed(Failure::Unreadable));
                        }
                        tree.archive = Some(indexed.info);
                    }
                    Ok((Err(failure), _)) => { tree.levels.insert(tree.root.clone(), Level::Failed(failure)); }
                    Err(_) => { tree.levels.insert(tree.root.clone(), Level::Failed(Failure::Unreadable)); }
                }
                tree.refresh(row_height());
                cx.notify();
            });
        })
        .detach();
    }

    fn read_tree_level(&mut self, path: String, cx: &mut Context<Self>) {
        let generation = self.generation;
        let Some(tree) = self.tree_mut(generation) else { return };
        let (listing, cancelled) = (tree.listing, tree.cancelled.clone());
        let target = path.clone();
        let work = self.load_work(move || Ok::<_, String>(read_local(&target, listing, &cancelled)));
        cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                let Some(tree) = this.tree_mut(generation) else { return };
                let level = match result {
                    Ok((Ok(read), _completion)) => {
                        tree.contents.extend(read.contents);
                        Level::Ready(read.folder)
                    }
                    Ok((Err(failure), _)) => Level::Failed(failure),
                    Err(_) => Level::Failed(Failure::Unreadable),
                };
                tree.levels.insert(path, level);
                tree.refresh(row_height());
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn toggle_tree_folder(&mut self, path: String, cx: &mut Context<Self>) {
        let generation = self.generation;
        let Some(tree) = self.tree_mut(generation) else { return };
        match tree.toggle(&path) {
            Toggle::Refused => return,
            Toggle::Changed => tree.refresh(row_height()),
            Toggle::Load => {
                tree.refresh(row_height());
                self.read_tree_level(path, cx);
            }
        }
        cx.notify();
    }

    /// A double-click selects the item alone in the source pane, inside its folder;
    /// the Viewer then follows that selection.
    pub(super) fn reveal_tree_item(&mut self, path: String, cx: &mut Context<Self>) {
        let (tab, Some(owner)) = (self.source.tab.clone(), self.owner.upgrade()) else { return };
        owner.update(cx, |app, cx| app.reveal_in_tab(&tab, path, cx));
    }

    /// The row counting hidden entries opens their folder in the source pane.
    pub(super) fn open_tree_folder(&mut self, path: String, cx: &mut Context<Self>) {
        let (tab, Some(owner)) = (self.source.tab.clone(), self.owner.upgrade()) else { return };
        owner.update(cx, |app, cx| app.open_in_tab(&tab, path, cx));
    }
}
