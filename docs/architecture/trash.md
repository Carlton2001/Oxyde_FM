# Trash

The Recycle Bin on Windows and the freedesktop trash on Linux, shown as one location, `trash://`. Moving items there is a deletion like the others ([operations](operations.md#partial-deletions)); this page covers browsing, restoring, emptying and the settings.

## Browsing

`trash://` lists the trashed items through `Platform::list_trash_items` (`commands/ops/linux_trash.rs` on Linux, `platform/windows_trash/listing.rs` on Windows). The list adds two columns, **Original location** and **Deleted at**, which sort (`SortColumn::OriginalLocation`, `SortColumn::DeletedAt`) and group (`GroupMode::Deleted`, `GroupMode::Origin`) only there; a new tab opened elsewhere falls back to sorting by name. There, the search field filters the list in place instead of starting a search (`TabState::quick_search_query`). Undo and Redo are hidden from the top bar.

On Windows the Shell may show an opaque `$R…` name: its `$I…` sidecar gives the original path, hidden extensions included (`platform/windows_trash.rs`). On Linux each item is `$trash/files/<name>` with its record in `$trash/info/<name>.trashinfo`, which is also its identity in the list (`platform/linux_trash.rs`). Copying or cutting from the trash keeps the physical path as identity and resolves names through `Platform`.

## Restoring

Restore puts each selected item back at its original path, independently: a failure does not stop the next ones, and the notice names the cause (`TrashRestoreFailure`).

- Nothing is overwritten: an item whose original path is taken stays in the trash (conflict).
- A missing original folder is not recreated.
- On Linux the content is moved back with a no-replace rename (ntfs-3g included), then the `.trashinfo` is removed and the item's entry dropped from `directorysizes`, the cache KDE reads. A trash and a folder on different file systems are refused, as the rename cannot cross them.
- On Windows the payload itself is moved back after its parents are held and checked (`platform/windows_trash_restore.rs`); a guessed name is never retried.

## Emptying and permanent deletion

Deleting inside the trash, or **Empty trash**, removes items permanently: `SHEmptyRecycleBinW` without the Shell's confirmation or progress on Windows, `linux_trash::purge` of every item on Linux.

## Settings

The trash window (`app/trash_settings/`) edits the system's own settings; Oxyde keeps none of its own.

- **Windows**, per volume, under `HKCU\…\Explorer\BitBucket\Volume\{GUID}`: `MaxCapacity` (MB) and `NukeOnDelete`, deleting without the Recycle Bin. The deletion confirmation reads this policy again just before it shows (`trash_settings/deletion_policy.rs`), so it says when a deletion will be permanent.
- **GNOME**: automatic emptying of items older than a number of days, `org.gnome.desktop.privacy` `remove-old-trash-files` and `old-files-age`; housekeeping applies it to every application's items.
- **KDE**: for each trash directory, the time limit, the size limit as a share of the partition and the action when it is reached (warn, delete the oldest, delete the largest), in `~/.config/ktrashrc`. KDE applies them when its own applications trash files.
- **Other desktops**: no settings are offered; the trash still works.
