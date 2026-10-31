# Symbolic links

Showing, opening, renaming, deleting, copying and moving a symbolic link on Linux. Windows junctions and reparse points keep their behavior: a link to a folder already has the folder attribute, and transfers refuse reparse points.

Deleting (trash or permanent) and renaming act on the link, never its target.

## Rules

- **Display**: a link to a folder behaves as a folder (navigation in the link's path, like Nautilus); a link to a file shows the file's size; a broken link stays a link. The icon says where the link leads (folder, file, nowhere) and the type "Broken link" replaces "Symbolic link". Properties show the target and describe the object it points to; a broken link's describe the link itself. Windows keeps a single link icon. `file_entry_from_metadata` follows the link outside Windows for type and size.
- **Folder sizes**: a link is never measured or traversed in a measure.
- **Copy, move**: like `cp -P` and Nautilus, the link is reproduced with the same target text, never followed.

## Copying

- *Selection*: a link, chosen or met inside a folder, becomes a link source (`SelectedLink`): identity of the link itself through the `O_PATH` descriptor, and target text read on that same descriptor (`readlinkat` with an empty path), so on the captured object, not the name. On Windows `Platform::source_link_target` returns `None` and reparse points stay refused.
- *Plan*: links are listed apart, counted as 0-byte items and validated with files against name collisions. A broken link at the destination occupies its name: it is a conflict (Replace, Skip), also in conflict analysis, where a link to a folder is one item, not a folder to merge.
- *Execution*, after folders and before files: identity and target rechecked through the held source parent, then `symlinkat` in the held destination parent, which refuses an existing name. Replace creates the link under a private name in the same folder then renames it over the destination; this rename fails on a folder, which is thus never replaced. A system without links (FAT, exFAT) fails the copy with the system error, as Nautilus does.
- *History*: the journal records each created link with source, target and identity (`HistoryCreatedLink`). Undo sets the link aside under a private name and removes it only if it is still the same link to the same target; otherwise it puts it back and fails. Redo recreates links from the same source if it still points to the same target and the destination is free. A replacement blocks undo, as for a file.

## Moving

`link_moves::move_link`: link rechecked, then renamed between held parents without replacing (`renameat2` `NOREPLACE`, or name check then rename on ntfs-3g, which refuses the option) and rechecked once moved; a link substituted meanwhile is put back. Across volumes, the link is recreated at the destination and the source removed only if still the same link; otherwise the copy is removed and the move fails. The journal records moved links (`moved_links`). Undo requires a free original location and the same link at the destination, and recreates the original folders. Redo repeats the move with the same checks.

## Archives: refused on purpose

Formats can store a link (tar, zip via the Unix mode, 7z), but Oxyde refuses link entries on opening, extraction and in the archive workspace, because a link extracted then followed by a later entry can write outside the destination. An archive containing a link would thus be unreadable by Oxyde itself, and archiving the target instead would silently pull in outside data. Compressing or copying into an archive a link, or a folder containing one, is refused before any write, with a translated message (`ArchiveLinkRefusal`). Lifting this requires extracting links last, never following them, with a target confined to the destination.

On Windows, symbolic links and junctions stay refused by transfers.

## Pitfalls

- Read the target on the captured link's `O_PATH` descriptor rather than its name: a substitution between two checks does not change the link read.
- A broken link always occupies its name: `exists()` misses it, `symlink_metadata` does not. Conflict analysis and replacement detection must use the latter.
- ntfs-3g also refuses `RENAME_NOREPLACE` for links; the fallback checks the name is free then renames, with a race window that system cannot close.
