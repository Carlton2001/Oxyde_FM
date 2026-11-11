# Oxyde GPUI Linux patch

Source: `crates/gpui` and `crates/gpui_linux` from Zed commit
`33c95853ed2b6956f339733c63a8220964ecbeb6` (the application's existing pin).
Upstream source and Apache license are preserved, as for `vendor/gpui_windows`. This is
the same dependency, not an upgrade; `Cargo.lock` only loses the two crates' source lines.

The manifests resolve workspace inheritance to the same versions and pinned Git packages,
leave out examples, tests and dev-dependencies, and allow `unexpected_cfgs`, which Zed's
workspace does. The `LICENSE-APACHE` symlinks are expanded to the repository's text.

Local source changes, all marked `Oxyde:`:

- `gpui/src/elements/img.rs`: animated images advance in inactive windows too. A Wayland
  hover popup never becomes active, so GIF and WebP previews stayed on their first frame.
  They stop while the window's frames are not shown (covered, minimized).

- `gpui/src/platform.rs`: `PlatformWindow::reposition_popup`, false by default.
- `gpui/src/window.rs`: `Window::reposition_popup`, which forwards to it.
- `gpui_linux/src/linux/wayland/window.rs`: its Wayland version, which updates the anchored
  popup's rectangle and reuses the existing `xdg_popup.reposition` path. A popup that only
  moved answers its configure with a bare commit instead of a redraw, which measured 20
  moves per second under Mutter 50 (a commit every 49.6 ms, 8.8 ms without the redraw).
  A resized popup holds its frames until the compositor's answer, and its bare commits until
  the new frame is on the surface: `wp_viewport`'s destination changes at the configure, so a
  commit in between would stretch the old buffer to the new size.
- `Window::popup_offset` (all three files): where the compositor put a popup, for converting
  its coordinates to its parent's.
- `gpui_linux/src/linux/wayland/client.rs`: a press closes the grabbing popups that do not
  contain its target, after the press is delivered. A grab leaves the client's own surfaces
  clickable without dismissing the popup.
- `gpui_linux/src/linux/wayland/{client,window}.rs`: a window opened while a popup holds the
  keyboard takes the popup's toplevel as parent. Upstream parents it to nothing, so Properties
  opened from a menu could fall behind the main window.
- `gpui_linux/src/linux/wayland/client.rs`: displays report their logical bounds from
  xdg-output. Upstream divides the mode by `wl_output.scale`, the next integer at a fractional
  scale: 2560x1600 at 125 % reported 1280x800 instead of 2048x1280. A roundtrip at startup
  makes them known before the first window opens.

- `PlatformWindow::activate_with_token` (`gpui/src/platform.rs`, `activate` by default),
  `Window::activate_window_with_token` (`gpui/src/window.rs`) and its Wayland version
  (`gpui_linux/src/linux/wayland/window.rs`): presents an xdg-activation token that another
  process received, so the running Oxyde comes forward when a second launch or a
  FileManager1 call hands it a folder. Upstream only requests a new token, which Mutter and
  KWin refuse without a recent input event.

- `WindowOptions::foreign_parent` and `WindowParams::foreign_parent` (`gpui/src/platform.rs`,
  `gpui/src/window.rs`), with `ForeignWindow::{X11, Wayland}`: another process's window, as
  xdg-desktop-portal names it, which the new window opens over, modal to it. X11
  (`gpui_linux/src/linux/x11/window.rs`) sets `WM_TRANSIENT_FOR`, the dialog type and the
  modal state before mapping; Wayland (`gpui_linux/src/linux/wayland/{client,window}.rs`)
  binds `zxdg_importer_v2` (and v1, used when v2 is missing), imports the handle, calls
  `set_parent_of` after any GPUI parent and adds a modal `xdg_dialog_v1`; the import is
  destroyed before the toplevel. A handle of the other protocol is ignored. Upstream has no
  way to parent a window to another process's, which the portal file chooser needs.

Reason: GPUI repositions a popup only when it resizes, so a tooltip could not follow the
pointer. Windows and X11 reject anchored popups and are unchanged. When updating GPUI,
compare both copies to the new upstream.
- `gpui_linux/src/linux/wayland/{client,window}.rs`: a window's mouse position is the pointer's
  only while the pointer is over it. Upstream gives every window the last location in the
  hovered surface's coordinates, so a submenu opened beside its parent showed a hovered row.
