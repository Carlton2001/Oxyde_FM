# Menus, tooltips and secondary windows

On Windows, menus and tooltips use owned GPUI windows; a native failure must reactivate integrated rendering without losing business state. On Wayland they are also separate windows, but anchored (see [tooltips](#tooltips-on-wayland) and [menus](#menus-on-wayland)). X11 refuses these windows: menus and tooltips stay in integrated rendering there.

## Choosing the entry point

| Surface | API / rule |
| --- | --- |
| Ordinary tooltip | `NativeTooltipExt::native_tooltip`; no direct `.tooltip(...)` in views |
| File or tree hover | `components::tooltip::hover` sessions; [previews](previews.md) |
| Interactive menu | `components::popup` with factory, validity condition and close callback |
| Path suggestions | `popup::show_passive`; focus and keyboard stay in the field |
| Secondary window | `components::tool_window`; business state in the calling view |
| Modal prompt (rename, compression, network drive, confirmations, failure notices) | Fixed `DialogKind` from `app/dialog_windows`; `Modal::standalone`, never an overlay in the main window |

An identifier reuses the existing window; a pending request is replaced by the latest. The factory rebuilds current state, with no frozen business copy. On close, read the current spec rather than a callback captured at first opening.

## Placement and events

- `PopupAnchor::point` sets a local position, `element` anchors under a control and `beside` places a side menu. Measure bounds at render; menus do not follow the cursor after opening.
- Submenus use the real bounds of rows after scrolling, without the shadow, and touch their anchor. Their logical owner is the parent menu.
- Route business actions with `popup::with_owner`. Convert coordinates and rectangles with `owner_point`/`owner_bounds`, DPI included. Scrollbars keep their owner and distinct identifiers per window.
- Do not forward the pointer by `Window::dispatch_event` to the source: its hit testing would make elements under the menu react. Call only the needed drag handlers after conversion; capture ends a drag outside the surface.
- Never call `Window::dispatch_event` in `App::open_window`'s constructor: the root is not installed. Initialize modifiers after return through the untyped handle.

Win32 calls go through `Platform`. Placement and activation that may call back into GPUI are deferred and grouped, never synchronous during a render or `Window::update`. Create near the owner to keep a consistent initial DPI.

### Owned windows

On Windows, Properties and dialogs (`set_window_owner`) stay above the main window, with no taskbar entry; Advanced Search, Duplicates, Operations, their file list and the detached Viewer are independent. Linux follows the same split: `WindowKind::Floating` sets the parent via `xdg_toplevel.set_parent`. Wayland cannot remove a window from the taskbar: that stays the desktop's choice. A window opened from a menu takes the menu's window as parent, the popup keeping the keyboard until it closes.

### Menus on Wayland

`popup/wayland.rs` translates the placement rules above into `xdg_positioner`; the compositor flips or slides the menu, and its answer gives its position, hence `owner_point` and `row_bounds` (`Window::popup_offset`, added to the GPUI copy). Mutter and KWin flip anchor and gravity, not the offset, so `flip_axis` gets both sides of the anchor rectangle to keep the same gap once flipped. This rectangle must stay in the parent window: within a margin of its edge, a flipped menu's gap grows by at most that margin. An opening side does not change on an open surface: it is replaced. The first image is empty, since a surface needs a buffer before taking its content's size; only the visible frame receives the pointer, not shadow margins.

The interactive menu takes an `xdg_popup` grab, which gives it the keyboard with no hook: the compositor closes it on a click outside the application or an application change, and Oxyde learns of this from the window closing. Two rules differ from Win32:

- A grab leaves the application's surfaces clickable without closing the menu: the GPUI copy closes menus outside the target once the press is delivered, since that press may be the button toggling them.
- Only the last menu may take a grab, on pain of a fatal protocol error: opening a menu closes those it does not descend from.

`show_passive` has no grab: the keyboard stays with the field.

## Fixed modal prompts

Prompts are real owned windows, in the Properties template (`tool_window::header`, shared footer, fixed width, height fitted to content). `Platform::configure_fixed_dialog` disables the owner while they are visible and locks their position at its center, bounded to its work area, with no moving, resizing or minimizing. `follow_fixed_dialog_owner` recenters them when the main window moves. The owner is reenabled before hiding so Windows gives it back activation. Closing by title bar or Alt+F4 goes through `request_fixed_close`, which respects busy operations; on release, the owner's focus returns to `focus_handle` if it still pointed at the prompt.

On Linux, `WindowKind::Dialog` gets the same blocking: the `xdg_dialog_v1` protocol marks the prompt modal, and GPUI ignores the parent window's input while the prompt exists, also where the compositor does not know this protocol. Mutter attaches it to the parent, centers it and moves it along; otherwise the prompt stays movable.

## Focus and closing

Register actions through `popup::keyboard::item`, with the same callback as the click, excluding disabled rows. Keep arrows, home/end, Enter/Space, Tab/Shift+Tab and Esc; editors keep their handlers. Business shortcuts route to the source.

All `popup` surfaces keep Windows activation and native focus in their owning form, with no shadow change on opening. Interactive menus keep their logical GPUI focus; a `WH_GETMESSAGE` hook limited to the UI thread redirects keyboard messages to the form's last visible interactive menu, before GPUI accelerators translate. This covers selectors, context menus, settings, submenus and editable fields. Closing a submenu returns the keyboard to the previous menu; closing the last returns it to the form and frees the hook. `show_passive` reserves the keyboard for the source field. Closing asks for no native activation.

Return focus to the logical owner only if the family stays active. Keep new focus set by an action. An owner click closes descendants before its action; changing application closes menus without reactivating Oxyde.

Before destroying an owner, close its descendants with `tool_window::close_transients`, first respecting any refusal from a busy dialog. `on_window_closed` stays a safety net. Suggestions cancel when their owner becomes inactive.

## Tooltips and geometry

Native tooltips are transparent, passive `PopUp` windows, with no activation or taskbar entry; clicks pass through. One GPUI shadow, with no DWM decoration, non-client frame or global topmost. Keep the prepaint origin given by the shadow margin.

The visible frame follows the pointer at 12 logical pixels on X/Y, flipped and bounded to the work area. Fixed placements keep the element's bounds. A transparent shadow margin adds no visible offset.

`ui/window_geometry.rs` remembers the restorable position and maximized state; `app/tool_geometry.rs` manages secondary families via `AppConfig::tool_window_placements`. Keep negative positions, ignore minimization and bound invalid geometry to available screens. Writes defer by 350 ms; closing forces the last save. Resetting preferences keeps these positions.

### Tooltips on Wayland

An application does not place its windows there: `hover/anchored.rs` opens an `AnchoredPopup` (`xdg_popup`), an input-less surface whose frame the compositor keeps on screen. No hidden popup exists: the surface stays open and empty between items of a session and closes at its end, since opening another costs several frames. A compositor shows each frame, so the bubble empties only after 80 ms with no hovered item, otherwise it would flicker at each gap between files. Its shadow is the menus', stronger than on Windows, to stand out from desktop windows. The GPUI copy ([README](../../vendor/gpui_linux/README.oxyde.md)) adds `Window::reposition_popup`, which moves the surface without redrawing: with a redraw, Mutter 50 limited it to 20 frames per second.

When the size changes, between two file types for example, the image stays the previous item's until the compositor answers, then the next arrives at once at the right size. Measured content asks for this size before presenting (`MeasuredContent::prepaint`); GPUI does not draw while the answer is missing and validates no bare move until the image is on the surface, as the compositor would otherwise stretch the old one to the new format (`wp_viewport`).

Mutter closes a popup no longer touching its parent window. Shadow margins keep it in contact, whichever edge the pointer is on. It may also close it on its own: `native_window::sync` then opens another at the next movement.

## File drag and drop

On Windows, the ghost uses an owned GPUI window, click-through, with no activation or taskbar entry. It follows the pointer and its work area, also outside the owner. It becomes temporarily topmost during the OLE session, then is destroyed on drop, cancel or owner close. Native placements are deferred and grouped through `Platform`. The cursor stays unchanged; the ghost shows the negotiated action or refusal. Release or cancel hides the ghost from `QueryContinueDrag`, before calling the foreign `Drop`, which may take a while. This hiding invalidates pending placements and late measures, and no placement message can reopen the surface. Session end separately keeps the transfer acknowledgment.

Internal interactions stay in GPUI. On leaving the window, passing over an overlaid foreign window or changing application, real paths verified beforehand in the background go to an OLE session, started by message after GPUI borrows are released. The internal drag is then removed; returning to Oxyde goes through the same external drop as other programs. Virtual archive members are not exported without preparation.

On Linux, in Wayland, GPUI hands the gesture to the compositor when the pointer leaves the window (`app/drag/outgoing.rs`). It offers paths as `text/uri-list`; the receiver copies or moves itself, as from Nautilus, and Oxyde sees the result through folder watching. The compositor shows no ghost, as GPUI provides none. Returning to Oxyde resumes the internal drag, hence its move. A selection is refused whole if it holds an archive member, a non-UTF-8 name, or comes from the trash: the gesture then stays internal. GPUI offers no files on X11, where it always stays internal.

`Platform::install_file_drop_target` replaces GPUI's Windows receiver to pass on modifiers, allowed effects and refusal of target-less areas. Folders, lists (details/grid), physical sidebar entries and tabs share the targets in `app/drag/external.rs`. Operations and conflicts stay in the usual asynchronous paths. A drop on the tabs' free space opens folders after background verification.

Oxyde performs incoming moves itself. The receiver signals an optimized move (`Performed DropEffect = NONE`, return `NONE`) so the foreign source does not delete files a second time. For an outgoing drag, only explicit confirmation of a non-optimized move asks the operation engine to remove sources. The session carries the whole selection in a single `CF_HDROP` object, without changing the clipboard. Virtual items another application provides only as streams are not accepted.

Linux keeps GPUI's incoming events and the shared targets. The outgoing OLE session and independent window are Windows-specific.
