# Oxyde GPUI Linux patch

Source: `crates/gpui` and `crates/gpui_linux` from Zed commit
`33c95853ed2b6956f339733c63a8220964ecbeb6` (the application's existing pin).
Upstream source and Apache license are preserved, as for `vendor/gpui_windows`. This is
the same dependency, not an upgrade; `Cargo.lock` only loses the two crates' source lines.

The manifests resolve workspace inheritance to the same versions and pinned Git packages,
leave out examples, tests and dev-dependencies, and allow `unexpected_cfgs`, which Zed's
workspace does. The `LICENSE-APACHE` symlinks are expanded to the repository's text.

Local source changes, all marked `Oxyde:`:

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

Reason: GPUI repositions a popup only when it resizes, so a tooltip could not follow the
pointer. Windows and X11 reject anchored popups and are unchanged. When updating GPUI,
compare both copies to the new upstream.
