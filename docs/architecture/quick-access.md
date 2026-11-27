# Quick access and opening

Pinned folders in the Quick Access section, pinning and unpinning, their refresh, the home folder heading them; on Linux, opening an item through the desktop's application, and the Properties system button. Cloud drives, network and trash are separate.

## Interface

`Platform` exposes `fn quick_access(&self) -> &dyn QuickAccess`, defined in `src/platform/quick_access.rs`:

```rust
pub trait QuickAccess: Send + Sync {
    fn items(&self) -> Vec<QuickAccessItem>;
    fn pin(&self, path: &str) -> Result<(), CommandError>;
    fn unpin(&self, path: &str) -> Result<(), CommandError>;
    fn watch(&self, revision: Arc<AtomicU64>) -> Result<Option<Box<dyn Send>>, String>;
}
```

- **Windows** (`windows_quick_access.rs`) delegates to `windows_shell` (verbs `pintohome`, `unpinfromhome`) and watches `Recent`.
- **Linux** (`linux_quick_access/`) reads and rewrites `$XDG_CONFIG_HOME/gtk-3.0/bookmarks`, the file Nautilus shares: local folders only, order and names kept, undisplayed lines (network locations) preserved, atomic rewrite like GTK. It watches the `gtk-3.0` folder, since GTK replaces the file at each change.

## Order

Dragging a pinned folder in the sidebar reorders it (`components/sidebar/reorder.rs`, `app/pinned_order.rs`). As a dragged tab, the folder is lifted at its row's width and the other folders slide aside (`reorder::shifts`, a `motion::glide` spring per row) to open the slot it targets and close the place it left, with no insertion marker. The order is Oxyde's own, kept in `AppConfig::pinned_order`; the system file manager keeps its order, and folders missing from Oxyde's list follow at the end in the system's order. The Windows taskbar menu lists the pinned folders.

## Home folder

The home folder opens the Quick Access section on every system, above the pinned folders, as in Nautilus, with the Lucide `house` icon, in the expanded sidebar and in the rail; it cannot be unpinned. An item with no name designates it, and the interface names it in the chosen language (`sidebar-home`).

## Cloud drives on Windows

`Platform::cloud_sync_roots` reads the Windows Explorer sync root registrations in the background. It uses the current user's SID to find the registered folder, resolves the display name from the registration, and shows each existing folder once above Drives. The sidebar refresh picks up newly registered roots. The Cloud drives setting also recognizes the former `show_onedrive` preference.

Within these folders, Cloud Files reports local availability and whether placeholders are in sync. Oxyde offers offline and online-only actions when the Windows Shell publishes the corresponding commands. Providers that expose a mounted volume appear under Drives. A client that exposes only an ordinary folder without a sync root registration can be pinned manually.

## Behavior outside Windows

- Opening waits for `xdg-open`'s verdict (3 s at most): a failure shows as on Windows; past that, the process is reaped in the background.
- `show_properties_dialog` does nothing: Oxyde's Properties window serves Linux.

## Pitfalls

- Respect `XDG_CONFIG_HOME` as soon as a feature writes a desktop file: the isolated test profile then protects real data with no other precaution.
- GNOME identifies a file by content: testing an open failure takes a binary file, not just an unknown extension.
