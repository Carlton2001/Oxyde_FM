# Plan: Oxyde as the portal file chooser

Working plan for the TODO in [Linux distribution](../guides/linux-distribution.md#opening-a-folder) ("Open and Save dialogs"). Read this file after each `/clear`, take the next unchecked step, and tick it once it is committed.

## Goal

Applications that go through `xdg-desktop-portal` (Flatpak, Firefox, Chrome, recent Electron) get Oxyde as their Open and Save dialog. The feature sits behind a Settings switch, off by default, and `install.sh --uninstall` cleans it up.

## Starting points in the code

- `src/platform/linux_instance.rs`: D-Bus service for `org.freedesktop.FileManager1` and the single instance per session. Reuse its connection and patterns.
- `src/platform/linux_startup.rs`, `src/ui/app/instance.rs`: startup and requests routed to the running instance.
- `src/platform/mod.rs`: `Platform` trait. System calls go through it.
- Tests: `src/platform/linux_instance/tests/`.

## Fixed decisions

- Files to install: `$XDG_DATA_HOME/xdg-desktop-portal/portals/oxyde.portal` (`org.freedesktop.impl.portal.FileChooser`) plus its D-Bus service file.
- `~/.config/<desktop>-portals.conf`: only the `FileChooser` line changes, and the previous value is restored when the switch is turned off.
- After each change, restart `xdg-desktop-portal`.
- No cursor changes. Strings in all five catalogs. Heavy work stays off the UI thread.

## Steps (one session each, commit at the end, no push without an explicit request)

- [x] **1. Minimal `OpenFile`** (effort high): `impl.portal.FileChooser` backend, `Request` object (Close), served by the running instance or by an Oxyde started without a main window; one-file picker; Settings switch; installing and removing the files plus `portals.conf`; uninstall. Test with `gdbus call` / `zenity`-style calls through the portal.
- [ ] **2. Multiple selection, folder, filters** (medium): `multiple`, `directory`, `filters`, `current_filter`, `current_folder`.
- [ ] **3. `SaveFile` / `SaveFiles`** (medium): `current_name`, `current_folder`, overwrite confirmation, `choices`.
- [ ] **4. Parenting to the caller** (high/xhigh): `parent_window` (`x11:` / `wayland:` handle). On Wayland, `xdg-foreign` needs a GPUI patch; otherwise keep an independent window.
- [ ] **5. Robustness** (high): timeouts, cancellation, behaviour when Oxyde crashes (the `gnome;gtk` fallback does not cover a failing backend), tests with Firefox, Chrome and a Flatpak.

## Notes between sessions

(Record here whatever the next session needs to know: choices made, traps, remaining TODOs.)

### After step 1 (2026-11-07)

- Where things are: `platform/linux_portal.rs` (files, `portals.conf` search in the portal's order, restart through systemd `RestartUnit`), `linux_portal/conf.rs` (pure editing of the FileChooser line, tested), `linux_portal/chooser.rs` (D-Bus `FileChooser` + `Request`, forwarding), `ui/app/file_chooser/` (window, browsing, Settings switch), `ui/app/main_window.rs` (main window opened on demand, quit rules). Reference: `docs/guides/linux-distribution.md` § Open and Save dialogs.
- Names: portal id `oxyde` / bus `org.freedesktop.impl.portal.desktop.oxyde`; debug builds `oxyde-devel` / `….oxyde.Devel`, with `current_exe()` as program. The service file starts `oxyde --portal`.
- Restoring the previous value needs no saved state: Oxyde only prepends `oxyde;`, so removing it gives back the line. Marker comments (`# Added by Oxyde for its file chooser.`, `# Written by Oxyde from <path>`) cover the line or file Oxyde created; `install.sh --uninstall` has the same logic in awk (`forget_file_chooser`).
- Interim forwarding: `directory=true`, `SaveFile`, `SaveFiles` go to the next portal of the line (bus name from its `.portal`), on a second connection without the 5 s `method_timeout`. Steps 2 and 3 replace these forwards with Oxyde's own handling; keep forwarding as the fallback idea for step 5.
- `multiple` is answered with one file; `filters`, `current_filter`, `current_folder`, `choices` are ignored. The window starts in the folder the previous one ended in, else home.
- Replies: the `Results` value holds a `Pending` guard that zbus drops only after writing the reply; `Platform::answering_file_chooser` keeps Oxyde alive after its last window (Linux quit mode is `Explicit`, `main_window::end_with_last_window`), and `finish_process_shutdown` waits 2 s for last replies.
- A caller that disconnects (e.g. `gdbus call`, which exits after getting its handle) makes the portal withdraw the request: the window closes at once. Test with a connected caller; a GLib Python script that calls `org.freedesktop.portal.FileChooser.OpenFile` and waits for `Response` works, also on a private bus (`dbus-run-session`, with `XDG_CONFIG_HOME`/`XDG_DATA_HOME` exported before it so the private bus and portal read the test files).
- Verified on 2026-11-07, Fedora 45 / GNOME 51 / xdg-desktop-portal 1.22.1, private bus: activation by the service file, `OpenFile` → window → `Close` → response 2 and exit; hand-over from `oxyde --portal` to a running Oxyde (`ServeFileChooser`); `SaveFile` forwarded to the GNOME backend and its `Close`; end-to-end through `xdg-desktop-portal`. **Not yet checked by the user:** choosing a file with the mouse and keyboard, the Settings switch on the real session (Strata is currently first there: the line becomes `oxyde;strata;gnome;gtk;`), Firefox/Chrome.
- Left for later steps: parenting (`parent_window` is ignored, the window is independent and gets focus only as the compositor allows), a backend that never answers a forward keeps Oxyde alive (step 5 timeouts), a non-UTF-8 folder name is shown lossily (the chosen path itself is decoded exactly).

### Richer window (2026-11-07, after step 1)

- The user found the first window too bare and chose to enrich the dedicated window rather than reuse the main one. A first version drew its own pieces and diverged from the main window (column widths, sort arrows, header labels, the path being edited): every piece is now a copy of the main window's component, adapted, and the main components stay untouched. Changing one of them means checking its copy here.
- Modules and their originals: `file_chooser.rs` (state, open, answer), `browse.rs` (history, listing, arrangement, selection), `arrange.rs` (pure filter, `workspace::sort_files`, type-ahead and arrows from `type_select.rs` and `selection.rs`, remembered `Options`; tested), `navbar.rs` and `crumbs.rs` (`top_bar.rs`, `breadcrumb_glide.rs`, `breadcrumb_fades.rs`), `location.rs` (the path edit; tested), `filter.rs` (`render_quick_search_control`), `details.rs` (`details.rs`, `headers.rs`, `details_row.rs`, `measurements.rs`), `grid.rs` (`grid.rs`, `grid_card.rs`), `status.rs` (status bar, `view_mode_switch`), `sidebar.rs` (`sidebar/rows.rs`, `volume_row.rs`, `tree.rs`, the resize handle of `app/render.rs`, `mount_volume`), `keys.rs`, `render.rs`.
- Left out on purpose: context menus, drags and drops, renaming, column filters, size bars and grouping, horizontal scrolling (the sidebar's width is bounded so the pane keeps its minimum), scroll-to-top, the sidebar's folder tree and rail.
- Tooltips use the main window's shared hover sessions (`tooltip::hover`, regions `Controls("file-chooser-files")` and `Controls("file-chooser-places")`, owned by the main window's entity): same delay, bridge, dismissal on scroll, press and window deactivation. Their regions do not carry the folder, so the render dismisses them when the folder changes.
- The sidebar observes the main window and follows its pinned folders, drives and unmounted volumes. A volume mounted here updates the main window's drives as its own mount does; a failure shows as the main window's toast, which is not visible when Oxyde runs for the portal only.
- `Options` (grid, sort column, direction, last folder, sidebar width) are saved as `Config::file_chooser_options` (JSON value, unknown content falls back to defaults) when the window answers or the user closes it; a withdrawn request saves nothing. Hidden files start from the main window's setting and are not saved.
- Folders always come first; folder sizes are not counted, so Size keeps folders by name. Filtering selects the first match so Enter chooses it; the filter's Up/Down arrive as `CompletionUp`/`CompletionDown` actions and move the selection.
- Grid columns and the details' widths come from the pane's width measured by a `canvas` in the previous frame (one extra frame on resize); thumbnails reuse `file_list::render_grid_thumbnail` under pane id `file-chooser-<entity>`, when the main window shows grid thumbnails.
- Step 2 should build multiple selection on `selected` (now a position in `shown`), and type filters on `arrange::arrange`.
