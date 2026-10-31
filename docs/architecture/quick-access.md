# Quick access and opening

Pinned folders in the Quick Access section, pinning and unpinning, their refresh; on Linux, the home folder among locations, opening an item through the desktop's application, and the Properties system button. OneDrive, network and trash are separate.

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

## Behavior outside Windows

- The home folder opens the Quick Access section, above bookmarks, as in Nautilus, with the Lucide `house` icon; it cannot be unpinned. An item with no name designates it, and the interface names it in the chosen language (`sidebar-home`).
- Opening waits for `xdg-open`'s verdict (3 s at most): a failure shows as on Windows; past that, the process is reaped in the background.
- `show_properties_dialog` does nothing: Oxyde's Properties window serves Linux.

## Pitfalls

- Respect `XDG_CONFIG_HOME` as soon as a feature writes a desktop file: the isolated test profile then protects real data with no other precaution.
- GNOME identifies a file by content: testing an open failure takes a binary file, not just an unknown extension.
