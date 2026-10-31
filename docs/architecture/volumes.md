# Volumes

Volumes live in a domain trait rather than `Platform`, and on Linux match Nautilus: mounted and unmounted partitions, mount on click, unmount and eject with authentication, refresh by signals. Network drives (`map_network_drive`, `is_network_path`…), `install_disk_image_autoplay_handler` (window integration, stays in `Platform`) and per-drive trash policy are separate.

## Interface

`Platform` exposes `fn volumes(&self) -> &dyn Volumes`, defined in `src/platform/volumes.rs`, implemented by `windows_volumes.rs` and `linux_volumes.rs`:

```rust
pub trait Volumes: Send + Sync {
    fn list(&self, skip_hardware_info: bool) -> Vec<DriveInfo>;
    fn roots(&self) -> Vec<DriveInfo> { self.list(true) }
    fn mount_points(&self) -> Vec<String> { Vec::new() }
    fn watch(&self, revision: Arc<AtomicU64>) -> Result<Option<Box<dyn Send>>, String>;
    fn unmounted(&self) -> Vec<UnmountedVolume> { Vec::new() }
    fn actions(&self, drive: &DriveInfo) -> DriveActions;         // pure, no system call
    fn mount(&self, id: &str) -> Result<String, VolumeError>;     // mounted path
    fn unmount(&self, path: &str) -> Result<(), VolumeError>;
    fn eject(&self, path: &str) -> Result<(), VolumeError>;
    fn is_ssd(&self, path: &Path) -> bool;
    fn physical_disk_id(&self, path: &Path) -> u64;
    // + the three disk image methods
}
```

- `DriveInfo` is identified by its path. `file_system` gives the format shown in Properties (NTFS, FAT32, exFAT, ext4…): `GetVolumeInformationW` on Windows, UDisks2 `IdType` and `IdVersion` on Linux, nothing for a FUSE mount.
- `actions(drive) -> DriveActions { unmount, eject }` decides the menu from the listed drive. The default, Windows's, gives `eject = is_usb()` and `unmount = false`. Linux follows the GTK sidebar: a removable or optical drive ejects, another volume unmounts; neither `/`, network mounts nor the fallback without UDisks2 offer an action.
- `UnmountedVolume { id, label, size, removable, file_system }` is a separate list in state (`unmounted_volumes`); Windows returns it empty. On Linux `id` is the UDisks2 block object; `mount` refuses any other D-Bus path.
- `VolumeError` distinguishes `Cancelled` (authentication canceled: no message), `NotAuthorized`, `Busy(applications)` (open files), `WindowsHibernated` and `Other(detail)`.

Default trait methods let Linux add capabilities with no Windows code.

## Linux: `src/platform/linux_volumes/`

- `udisks.rs`: `zbus::blocking` client to `org.freedesktop.UDisks2` (`GetManagedObjects`). `zbus` and `rustix` are already locked by GPUI.
- `inventory.rs`: pure function from UDisks2 objects to `DriveInfo`, tested on fixtures, like GVfs. `HintIgnore` hides EFI, recovery and swap. A mounted volume appears only under `/run/media/$USER/`, `/media/` or the home folder, with no hidden component. `/` comes first ("Root"), then internal disks, removable, optical, each by name (natural sort); unmounted volumes follow the same order. `HintSystem` is not used: UDisks2 sets it on every internal disk, `Data` included. Optical → `cdrom`, removable or USB → `removable`. Capacity through `rustix::fs::statvfs` in detailed mode, except network mounts. `media_type` stays empty.
- `inventory::unmounted`: filesystems (`IdUsage = filesystem`) with no mount point, like GVfs's volume monitor. `HintIgnore` hides them, a whole partitioned disk gives way to its partitions, an `/etc/fstab` entry outside user locations stays hidden except with `x-gvfs-show` (`x-gvfs-hide` always hides). `Filesystem.Mount` with no option returns the mount point; ntfs-3g reports a hibernated or fast-started Windows (`WindowsHibernated`).
- `actions.rs`: `Filesystem.Unmount` with empty options, so `auth.no_user_interaction = false`: polkit has GNOME show its dialog if needed, and the call blocks the background thread until answered. Eject unmounts all the disk's mounted partitions, then `Drive.Eject` for optical media, otherwise `Drive.PowerOff` when possible (safe removal, as on Windows), else `Drive.Eject`. UDisks2 errors map `NotAuthorizedDismissed` → `Cancelled`, `NotAuthorized*` → `NotAuthorized`, `DeviceBusy` → `Busy`.
- `holders.rs`: on `Busy`, like GNOME's "Volume busy" dialog, walks `/proc` (working directory, root, executable, descriptors, mappings) and names the user's processes holding the volume; the message adds "Open in: nautilus." Nautilus is only told of unmounts started by GNOME; otherwise it keeps its folders open.
- `watch.rs`: a thread `poll`s the `POLLPRI` flag the kernel raises on `/proc/self/mountinfo` at each mount or unmount in the namespace, from UDisks2, `mount` or a network client, bumping the revision. The list refreshes at once; polling goes from 2 s to 30 s as a fallback only. Closing the guard's pipe ends the thread. Works without UDisks2.
- `signals.rs`: UDisks2 `InterfacesAdded`/`InterfacesRemoved` and `PropertiesChanged` on `Filesystem` or `Block` bump the same revision, covering volumes plugged in but unmounted, and state UDisks2 records after the kernel. Job progress and SMART measures are ignored. The thread has its own connection; a blocking iterator does not wake, so it stops at the signal after the guard is released.
- `disks.rs`: physical disk of a path, for the pace of copies, searches and duplicates. The deepest mount containing the path gives its `/dev/…` source; `/sys/dev/block/MAJ:MIN` leads to the partition, its parent to the disk, and a device mapper volume (LUKS, LVM) to its first underlying device. Id `(major << 20 | minor) + 1`, `queue/rotational = 0` for SSD, host hash with the top bit for a network share, 0 if unknown (tmpfs, overlay). Answers are kept 10 s per filesystem (`st_dev`). USB sticks declare themselves rotational, so they stay on the cautious path.
- `fallback.rs`: without UDisks2 (WSL, container), all non-virtual `mountinfo` mounts (`mounts.rs`, escapes decoded), with no mount or eject; with UDisks2 it only adds network mounts.
- GVfs shares: see [Linux network](linux-network.md).

## Windows

`WindowsVolumes` in `windows_drives.rs` delegates to the existing code. On an ejection veto, `windows_eject_holders.rs` reads event 225 of `Microsoft-Windows-Kernel-PnP` in the System log, the only place Windows names processes, those of other sessions included, and returns `Busy(applications)`. It keeps the ejected node and request time, and retains `System` only if alone. With no readable event, the error stays `Other` with the PnP code.

## Interface

- Linux names a drive by its label alone ("Root", "Data"), like Nautilus and Dolphin, in the sidebar, tabs, properties and trash list (`format::drive_name`); the tooltip and Properties' Location give the mount point. Without a label, a volume is called "487 GB Volume" from its partition size (`DriveInfo::volume_bytes`, key `unlabeled-volume`), rather than the UUID naming its mount folder; named volumes come first. Windows keeps "Name (C:)".
- "System properties" exists only on Windows.
- Unmounted volumes (`SidebarItem::Volume`, `sidebar/volume_row.rs`) sit in the drives section after mounted ones, with dimmed icon and label and a "format · size" tooltip. Left click mounts in the background (`mount_volume`), then navigates to the path. Cursor unchanged, no context menu, absent from the collapsed rail.
- WSL distributions (Windows): a "Linux" row (`SidebarItem::Linux`, `box` icon) above Network, absent with no distribution. `wsl_distributions` reads the `Lxss` registry: reaching `\\wsl.localhost\<name>` starts the distribution, so neither list nor icon touch that share before a user action. `\\wsl.localhost` cannot be enumerated: `list_network_shares` answers from the registry and it is not watched. "Expand all" does not enter `/proc`, `/sys`, `/dev`, `/run`, `/mnt`.
- Menu: "Unmount" (`eject` icon) if `actions.unmount`, "Eject" (`unplug`) if `actions.eject`. `release_drive` handles both: watches are released before the call, otherwise Linux answers `Busy`; a canceled authentication shows nothing.
- Keys `drive-unmount`, `drive-unmount-failed`, `drive-holders`, `drive-mount-failed` and `drive-mount-hibernated` in all five catalogs.

## Decisions

- Partitions shown: those Nautilus shows, per UDisks2 hints, including Windows partitions not marked `HintSystem`.
- `/` is named "Root", as in Dolphin. Linux conventions take priority over Windows ones.

## Pitfalls

- A GPUI row cannot read the application during its render: state goes through the presentation.
- Nautilus keeps its folders open when another program unmounts.
