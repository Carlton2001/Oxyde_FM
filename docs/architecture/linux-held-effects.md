# Effects through held directories on Linux

Shared code prepares each file operation by holding its parent directories and verified objects (`HeldParents`, selection, staging), then asks `Platform` to act relative to them. This covers copying, moving, publishing, cleaning up and undoing. A folder renamed or replaced between check and effect receives nothing, and a substituted file is neither renamed nor deleted. Code: `linux_held.rs`, behind the existing `Platform` methods. Windows acts through its handles.

## Rules

- **Relative to held directories**: every named effect goes through the held directory and the last component (`openat`, `mkdirat`, `renameat2`, `unlinkat`), without following a link at the last component.
- **Held object, wherever it is**: renaming or deleting a verified object acts on it at its current location, read from the kernel (`/proc/self/fd`), then checks its identity (`dev`, `ino`) in that folder.
- **Deletion without substitution**: Linux cannot delete "this name if it designates this object". The entry is first renamed aside in its folder, its identity checked, then deleted; another object is put back intact.
- **Checked rename**: identity checked before, then on arrival; an object that arrived in the original's place during that brief interval is put back under its original name and the operation fails.
- **Exclusive publication**: relative `RENAME_NOREPLACE`; on FUSE (ntfs-3g), which refuses it, a hard link then checked removal of the old name; without hard links, checking that the name is free then renaming, the only remaining window. Tested on vfat, exFAT, NTFS, tmpfs and ext4 `casefold` (`OXYDE_TEST_PUBLISH`).
- None of these rules is a transaction: they guarantee Oxyde acts only on objects it verified, not that names stay frozen.

## Remaining gaps

- On exFAT, a deletion checks the entry then deletes it directly: the kernel gives a folder a new creation date as soon as an entry is renamed in it, which would invalidate its history proof. A substitution remains possible in this very short interval.
- Without `RENAME_NOREPLACE` or hard links, publication checks the name then renames.

## Pitfall

ntfs-3g no longer describes an `O_PATH` descriptor whose name has gone (`ENOENT`): identity is then read from `/proc/self/fdinfo` (inode) and `mountinfo` (device).
