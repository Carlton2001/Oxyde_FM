# Access without read, and Linux deletion

The rights an operation really needs on Linux. Moving or renaming an item changes an entry in its folder: it takes writing to the folder, not reading the item. Traversing a folder takes `x`, not `r`. A recursive deletion must be able to pause or cancel between items. Windows is unchanged.

## Rules

- **Held directories** (ancestors, parent, destination, source): opened for reading if allowed, otherwise `O_PATH`. The descriptor is enough for identity, relative calls (`openat`, `renameat`, `unlinkat`) and mounting; only listing or changing attributes needs read, and these then fail like `cp`.
- **Moved or renamed file**: `O_PATH | O_NOFOLLOW`, without reading. Content is read only if the move becomes a copy, across disks.
- **Recursive deletion** (`linux_file_tree.rs`): explicit descriptor-based traversal, without following links.
  - Pause and cancel are checked before each item.
  - Each folder is removed after its content, through its held parent.
  - Another filesystem mounted inside is not visited: its removal fails and nothing is deleted there, like `rm -r --one-file-system`. Unmount it first.
  - The first failure stops the selected item and is reported.
  - An unreadable subfolder is not forced open: failure, like `rm -r` and Nautilus.

The result matches `mv`, `cp` and Nautilus: an unreadable file moves and renames on its disk; a `-wx` folder moves but does not copy (that would need listing); a `--x` folder gives access to what it contains; copying into a `-wx` folder works. Moving a folder takes writing into it (its `..` entry changes), so `mv` also fails on a `000` folder.

## Turbo and parallelism

Parallel deletion slows Btrfs, whose locks serialize deletions, and speeds up ext4 (2–3× on small files, NVMe, warm cache; about 10% slower on Btrfs). So Turbo deletes as on Windows (up to 4 selected items at once; in a single selected folder, its subfolders on up to 4 workers), except when an item's folder is on Btrfs: one worker, Turbo or not. `Platform::parallel_deletion` (true by default) chooses this.

A flat folder of files does not change: its files are deleted one by one by the thread listing it, as on Windows. Parallel traversal keeps the same guarantees: each worker checks pause and cancel, a failure stops the others at their next checkpoint, and the selected folder goes last.

## Running the special tests

- Mounted filesystem: `OXYDE_TEST_MOUNTED=<folder>:<mount inside>`, for example under `unshare --map-root-user --mount` after `mount -t tmpfs none <mount>`.
- ACLs: `OXYDE_TEST_ACL=<prepared folder>` (layout in the test), under another user: `unshare --map-auto --map-root-user --mount`, with the folder bound on `/mnt` to avoid traversing the home folder, then `setpriv --reuid=1000`.
