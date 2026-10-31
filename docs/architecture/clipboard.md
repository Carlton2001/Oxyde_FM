# File clipboard

Copying, cutting and pasting a file selection (Ctrl+C/X/V, Paste button) and the button's state. Copied text ("Copy path", names) stays with GPUI.

## Linux constraint

On Wayland only the active application reads or fills the clipboard, through its own connection: for Oxyde, GPUI's. GPUI Linux exchanges only text and images there (`gpui_linux/src/linux/wayland/clipboard.rs`); file lists only travel by drag and drop. GNOME 50 offers no background clipboard protocol (`ext-data-control`). Nautilus exchanges files as `x-special/gnome-copied-files`.

Zed's `project_panel` does the same on every platform: an internal clipboard distinguishing copy from cut, paths as text in the system clipboard, and on paste first the files GPUI reads from another application (`ClipboardEntry::ExternalPaths`, never filled on Linux), then the internal clipboard.

## Interface

`Platform` exposes `fn file_clipboard(&self) -> &dyn FileClipboard`, defined in `src/platform/file_clipboard.rs`:

```rust
pub trait FileClipboard: Send + Sync {
    // Ok(true): the interface also writes the paths as text.
    fn set_files(&self, paths: Vec<String>, cut: bool) -> Result<bool, String>;
    // `text`: system clipboard text, which makes a selection offered as text stale.
    fn get_files(&self, text: Option<String>) -> Result<(Vec<String>, bool), String>;
    fn has_file_format(&self) -> bool;
}
```

- **Windows** (`windows_file_clipboard.rs`) delegates to `clipboard.rs` (`CF_HDROP`, `Preferred DropEffect`).
- **Linux** (`linux_file_clipboard.rs`) keeps the selection in memory, for all the process's windows. A cut selection pastes only once: its sources have moved. When `wl-copy` is installed, a `wl-copy --foreground` process offers it in Nautilus's format and exits as soon as another selection replaces it, which makes the selection stale. On paste, a stale selection gives way to `wl-paste`, which reads files copied or cut elsewhere (`linux_file_clipboard/wayland.rs`).

## Behavior outside Windows

- Without `wl-copy`, copying or cutting writes the paths as text through GPUI, one per line. With it, the clipboard holds only the files, and "Copy path" remains the way to get text.
- Paste first takes files GPUI provides from another application (copy), then Oxyde's selection while the system clipboard still offers it (without `wl-copy`, while its text is what Oxyde wrote). Otherwise, files read by `wl-paste`.
- Without `wl-copy`, after text is copied elsewhere the Paste button stays active until the first paste; with it, the button greys out at the next poll. It does not activate for files copied elsewhere, since detecting them would run `wl-paste` at each poll, but Ctrl+V pastes them.

Under GNOME, lacking a background clipboard protocol, `wl-copy` takes focus for a moment: the window flickers at each copy or cut and open menus close, with no lost focus. This is accepted, because exchanging with the desktop's file manager matters more. It does not happen under KDE and wlroots compositors, where `wl-copy` uses `ext-data-control`.

## Pitfalls

- Test on an NTFS volume, not just Btrfs: some copy engine defects only appear with FUSE.
- GPUI limits can be worked around outside GPUI: a helper process (`wl-copy`) carries what GPUI's Wayland loop cannot offer.
