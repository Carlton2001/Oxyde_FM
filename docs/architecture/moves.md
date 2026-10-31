# Linux moves

Moving and renaming on Linux, like Nautilus: a folder moved on its volume, and a rename or trash restore on a FUSE volume. Cross-volume moves copy then delete. Windows folders still move item by item.

## Rules

- A selected folder is renamed whole when its name is free in the destination and it shares mount and filesystem with it (same mount id and device, through `statx`): the condition for the kernel to accept the rename (GIO's `g_file_move` does the same). The plan decides, and the rename happens only after all destinations validate, like other effects. Merging into an existing folder, two selected folders of the same name, another volume, the trash and archives keep the item-by-item path.
- The folder renamed is the one the selection verified: its identity is checked before and tracked after.
- Undo and Redo rename the whole folder through the same primitive, by identity, never replacing, and put back already renamed folders if the rest fails (journal `renamed_dirs`, `history_renamed_dirs.rs`).
- On a volume refusing `RENAME_NOREPLACE` (ntfs-3g answers `EINVAL`), the rename checks that the name is free then renames: a race window remains that this system cannot close. Files keep their hard-link fallback, which has none. `linux_rename.rs` is shared by item rename, trash restore and links.

`Platform::same_mount` is `false` on Windows. Code: `linux_rename.rs`, `copy/whole_moves.rs`, `history_renamed_dirs.rs`. NTFS rename tests use `OXYDE_TEST_NTFS`.

## Pitfalls

- `st_dev` is not enough to predict `EXDEV`: Btrfs subvolumes share a mount with distinct devices, bind mounts the reverse. `statx` gives both.
- An effect decided during planning must wait for all destinations to validate: renaming during the plan left a folder moved when another source collided.
- Two selected folders of the same name merge; renaming them whole would make the second collide with the first.
