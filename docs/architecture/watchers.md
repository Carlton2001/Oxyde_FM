# Folder watching

For each watched folder (tabs, expanded folders, drive roots, live sizes), the choice between system events and `notify` polling. The change queue, list refresh and folder sizes are separate.

## Why it differs by platform

On Windows, polling USB sticks avoids holding handles that block ejection. On Linux, inotify holds nothing open and does not prevent unmounting; it also sees changes made locally on a FUSE mount (ntfs-3g). It does not see those from another machine on a network share.

## Interface

`Volumes` has a default method carrying the rule:

```rust
/// Whether `path` is watched by polling rather than system events.
fn polls(&self, path: &str, drives: &[DriveInfo]) -> bool;
```

- **Windows**: the default, USB drive or unknown media type.
- **Linux**: the deepest mount containing the path decides. Polling for network mounts (`drive_type == "remote"`: NFS, CIFS, sshfs…), events everywhere else, USB sticks and NTFS included.

If inotify fails (`max_user_watches` reached on a very large tree), the error `DirectoryWatcher` already handles triggers a reread.

**Overflow at installation.** `notify` installs a recursive watch by reading each subfolder, and each read produces an open event. Past about 16,000 folders the kernel queue (`max_queued_events`) overflows before installation finishes, and an overflow asks to remeasure everything. An overflow reported less than a second after installation is therefore ignored: the lost changes date from installation, which the gap check in `reconcile_folder_size_watcher` already covers.

Code: `drive_watchers.rs`, `drive_watchers/changes.rs`, `volumes.rs`.

## Pitfall

Moving from polling to events also changes event volume: large trees overflow the kernel queue right at installation.
