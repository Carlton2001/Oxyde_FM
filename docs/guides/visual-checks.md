# Checking the interface in a real session

Pick the scenarios the change affects, on private fixtures. Note revision, Windows/target, theme, language, zoom, steps and result. A compile or Win32 test does not prove GPUI rendering.

| Change | Useful checks |
| --- | --- |
| Components, forms | Light and dark themes, five languages including long labels, zoom, small windows, Tab/Enter/Esc, errors and disabled fields |
| Menus, windows | Submenus after scrolling, capture outside the surface, text fields/clipboard, owner click, Alt+Tab, secondary window, several DPIs; nothing reacts under the menu |
| Previews | Transparent/animated images, PDF, video; fast hover, return on cache, modified source, scrolling, closing, no stale result |
| Viewer | Eye button, selection followed across panels, previous/next, no tab merging either way, moves by split; detach, resize, audio/video and pause/resume, file change, end and close |
| Operations | Progress, pause/resume/cancel, conflicts, partial receipts, history after restart, final graph |
| Archives | Navigation, extraction, creating/rebuilding the four writable formats, transfers, collisions, readable refusals and recovery |
| Distribution | Clean install and moved portable, caches/configuration, workers present, opening files and shutdown |

Do not provoke failures on personal documents or empty the real Trash for a test. If a small operation ends too fast to try pause, say so rather than presenting it as tested.

## Opt-in native tests (Windows)

These are excluded from the ordinary suite. Rebuild the binaries first and use a private `TEMP`/`TMP` under `target/`, as `test-windows.ps1` does, with ordinary Windows rights. An agent sandbox may refuse handles the file engine needs; run them outside it if so.

```powershell
# Drag ghost follows Ctrl/Shift without pointer movement
cargo test --offline --locked --bin oxyde native_drag_ghost_tracks_modifiers_without_pointer_movement -- --ignored --test-threads=1
# External drop of two temporary files: copy then move, no duplicate preparation
cargo test --offline --locked --bin oxyde native_external_multiselection_copies_then_moves_once -- --ignored --test-threads=1
```

The ghost test covers GPUI routing and ghost notifications, not Windows delivering physical keys or how colors look. Keyboard handlers belong to the focus-tracked root: a child `canvas` has its own dispatch node and does not receive events meant for that root.

`platform::windows_file_drag` unit tests exercise COM objects without moving the pointer or using the clipboard. They do not replace a real trial between Oxyde, Explorer and a third-party application. Also check Esc/right click, return into Oxyde after leaving, long names, screen/DPI change and closing during a drag. Use test files only for moves.

`native_viewer_media_*` tests check the Windows player on a private WAV and, if `OXYDE_VIEWER_TEST_MEDIA` is set, a private audio/video file. `native_viewer_renders_tracks_selection_and_detaches` exercises GPUI rendering, selection tracking after detaching, panel retention, preview return on window close, and tab-linked closing. They prove neither audibility, perceptual sync nor the full codec/DPI/theme matrix.

Automated tests cover timeouts, faults and hostile workers better; manual checks cover presentation, focus and interaction. Overall user feedback does not prove every variant: record its actual scope.
