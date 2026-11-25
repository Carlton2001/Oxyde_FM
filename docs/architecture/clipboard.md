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
    // The type other applications offer files in, checked against the interface's clipboard.
    fn external_files_type(&self) -> Option<&'static str>;
}
```

- **Windows** (`windows_file_clipboard.rs`) delegates to `clipboard.rs` (`CF_HDROP`, `Preferred DropEffect`). When the clipboard offers only virtual files (`FileGroupDescriptorW`), or `CF_HDROP` paths none of which exists yet, paste falls back to the shell's own Paste verb on the target folder (`windows_system_paste.rs`): Windows then shows its progress and conflict dialogs. A selection of which only some paths are gone stays Oxyde's paste, which reports them. VMware Tools needs this: a host file arrives as virtual file, a host folder as a staging path (`%TEMP%\vmware-<user>\VMwareDnD\…`) that it fills only when the shell pastes; reading `CF_HDROP` or `IFileOperation::CopyItems` does not trigger it. Its drag and drop already gives real paths.
- **Linux** (`linux_file_clipboard.rs`) keeps the selection in memory, for all the process's windows. A cut selection pastes only once: its sources have moved. The interface writes the paths as text; under Wayland, the patched GPUI also offers them as files (`text/uri-list`, `x-special/gnome-copied-files`, `application/x-kde-cutselection`). On paste, a selection whose text was replaced gives way to `wl-paste`, which reads files copied or cut elsewhere (`linux_file_clipboard/wayland.rs`), killed after 3 seconds if the clipboard owner does not answer.

## Behavior outside Windows

- Copying or cutting writes the paths as text through GPUI, one per line.
- Paste first takes files GPUI provides from another application (copy), then Oxyde's selection while the system clipboard's text is what Oxyde wrote. Otherwise, files read by `wl-paste`.
- After text is copied elsewhere the Paste button stays active until the first paste. Under Wayland it also activates for files another application offers in Nautilus's format: GPUI keeps the types of the current offer, and `App::clipboard_offers` checks them at each poll without reading the clipboard or starting `wl-paste`. The offer only reaches Oxyde while one of its windows has the keyboard, as Wayland sends it.

## Pitfalls

- Test on an NTFS volume, not just Btrfs: some copy engine defects only appear with FUSE.
- GPUI limits can be worked around outside GPUI, as `wl-paste` reads what GPUI's synchronous Wayland read would block the interface on.
