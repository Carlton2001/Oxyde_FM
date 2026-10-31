# Linux copies

Copying a file on Linux, including the copy in a cross-volume move: where it reads from, how it writes, and what it keeps from the original file and folders. Staging, the BLAKE3 re-read of the output, exclusive publication and history are shared. Windows native copy goes through `CopyFileEx`.

## Rules

- Read only through the verified descriptor when the caller provides one. A file substituted between check and copy is never copied in its place.
- Let the kernel copy (`copy_file_range`): block sharing where the filesystem allows, kernel-side copy otherwise, with a fallback loop for systems that refuse (ntfs-3g, FAT, old kernels). Progress, pause and cancel are checked between chunks.
- Keep mode, access and modification dates and ACLs, like `cp -p`, and `user.*` extended attributes, like Nautilus. Not the owner, which requires root. SELinux context and other attribute namespaces are not copied: the destination gets its own, as with `cp`. A source without ACLs keeps in the copy the default ACL inherited from the destination, as in Nautilus.
- Extended attributes are written before the mode: writing `user.*` needs write permission, and the mode applied afterwards keeps the ACL up to date.
- setuid and setgid are kept only if the copy has the same owner as the original.
- A folder created by the copy gets the original's attributes once its content is copied, deepest first, through its held descriptor. Its owner always keeps read, write and traverse (`mode | 700`, applied after the ACL), otherwise neither Undo nor the rest of the copy could write there; group and others are taken as is. An existing folder being merged into is never modified.
- An attribute the destination cannot keep (FAT, some mounts) does not stop the copy: it is logged by name.
- Undo and Redo restore these attributes on folders they recreate, from the transaction journal (`history_directory_attributes`).

The same applies to a same-volume move that recreates folders; whole-folder moves are in [moves](moves.md).

Code: `linux_file_copy.rs`, `linux_extended_attributes.rs`, `copy/directories.rs`, `history_directory_attributes.rs`.

## Known limits

- A restored access date survives only until the first read, as with `cp -p`: a copy always has a more recent change date, so `relatime` updates access. Nautilus reads the file when its Properties open. Oxyde re-reads its own copies (verification, Undo) with `O_NOATIME` and resets the date after verification, because ntfs-3g ignores `O_NOATIME`.
- NTFS mounted by ntfs-3g without `permissions` keeps no mode: every file appears as `777`, hence executable. It keeps `user.*` (as named streams), not ACLs. FAT keeps neither mode nor extended attributes.
- A folder containing a read-only subfolder (`550`) can only move if renamed whole, on its mount. Across volumes its files cannot leave the source, as with Nautilus.

## Pitfalls

- Writing ACLs before the mode avoids parsing their format: the kernel recomputes owner, mask and other entries at `chmod`, which keeps the owner's full access.
- NTFS trials use an image: `udisksctl loop-setup` then GNOME's automount, without touching the user's disks.
