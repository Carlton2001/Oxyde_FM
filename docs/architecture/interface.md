# Interface conventions

Business state and actions belong to `src/ui/app/`. Components receive data and callbacks and own only their local focus, scroll or expansion state. `src/ui/elements/` provides primitives. The [development guide](../guides/development.md) covers splitting.

## Themes and dimensions

- Get the theme through `cx.theme()` and use `OxydeTheme` tokens. Add a token to every palette if a shared role is missing; keep local values for component-specific effects.
- The thirteen built-in themes are declared by `ThemeId::ALL`, each by six seed colors (`theme/seed.rs`); every other token derives from them. User themes (`AppConfig::custom_themes`, `models/custom_theme.rs`), saved in `themes.json` beside `config.json` so a preferences reset keeps them, store the same six seeds and the built-in theme they started from. The theme window (`app/theme_window`) applies a theme on selection and previews a draft in every window through `OxydeApp::preview_theme` until it is saved or dropped. File icon colors are computed from each theme's seeds in `theme/file_colors.rs` (fixed hue per category, lightness for 3:1 contrast on the panes, chroma from the accent, folders on the accent), mapped by `components/icons/colors.rs` and applied by `render_file_icon`. The color vision setting (`AppConfig::color_vision`: typical, red-green, blue-yellow) swaps the category table for one searched to stay distinct under the matching deficiency simulation, and moves success off green for red-green; it applies to every theme.
- Express dimensions in `rems()`, initial reference 16 logical pixels. Use `ui::scale::rem_pixels` for a `Pixels` API, `logical` for older values at scale 16. These conversions stay on the UI thread.
- Coordinates from GPUI are already in pixels: do not convert twice. Content zoom does not resize the native window.
- The default weight is Regular (400), from `ui/style.rs`. Separately rendered roots apply it too. Another weight must be an intentional variation. On Linux, GPUI ignores the variable font's weight axis, so Inter Medium, SemiBold and Bold are embedded as static fonts (`assets_linux/fonts`, `ui/assets.rs`).

## Shell shortcuts

- **Terminal** (Linux, Ctrl + . or the folder menu): opens the folder selected alone, else the folder shown, in `xdg-terminal-exec` when present, then the desktop's own terminal (Konsole on KDE, Xfce or MATE's), then Ptyxis, GNOME Console, GNOME Terminal and other common terminals (`platform/linux_terminal.rs`). Not offered in the trash or inside archives.
- **PowerToys Peek** (Windows): when Peek is installed and enabled, its own shortcut (Ctrl + Space by default, or Space when Peek allows it) read from PowerToys' settings opens the selected file in Peek (`platform/windows_peek.rs`, `app/shell_features.rs`). Oxyde's own previews do not need it.

## Window buttons

- `components/window_controls.rs` draws minimize, maximize and close for every Oxyde title bar, in the style `Platform::window_control_style` picks: Windows captions, Adwaita discs under GNOME and other Linux desktops, bare Breeze glyphs (Lucide chevrons and diamond) under KDE.
- The desktop's button layout (`cx.button_layout()`) decides which buttons show, their order and their side. Buttons on the left come before the logo and settings button, which keep their place.

## Languages and formats

`cx.i18n()` resolves `Message` variants. For any user text, add or reuse a key and its translations in `locales/{fr,en,de,es,it}.ftl`, then check catalogs and parameters.

Keep whole sentences in Fluent, with numeric parameters for plurals. Paths and user names stay literal data; strip bidirectional isolates only if the result becomes a system path. Persistent errors keep their key so they follow a language change. Technical details stay in logs.

`ui/format.rs` centralizes numbers, sizes and dates. Language and the persisted date format are separate from the regional system locale. Messages fall back to French; a formatting failure is logged.

## Reusing components

Checkboxes use `elements::checkbox::checkbox`: 1 rem square, 1/16 rem outline, 1/4 rem radius. At rest the neutral outline mixes surface and secondary text, staying less present than labels. Checked uses an accent background and outline with a centered check. Partial keeps the accent outline but reduces the accent background to 20% over the neutral surface, with a text-colored dash. Views keep click areas, focus and actions. Toggles and plain choice marks keep their distinct roles.

Theme palettes pass through `theme/legibility.rs`: pane borders, menu outlines, secondary button fills, ghost and row hovers are pulled toward the text until they reach a minimum contrast against their surfaces (1.18, 1.3, 1.2, 1.15). Add a palette by its original colors; do not hand-tune these tokens per theme, raise the floor instead.

Single choices with few, short options (archive format and protection, view mode, tabs of a secondary window) use `elements::segmented`: one frame, shared with the − value + stepper, 2.25 rem high as the bars and fields, of which 0.125 rem above and below is an outer margin keeping it off its parent (options 1.625 rem, inside a 0.125 rem padding), square as the bars' buttons (`SQUARE` in `segmented.rs` rounds the frame and its options), filled with the text color at 5% and outlined at 16% so it contrasts on every theme; the chosen option sits on `selected` in accent text. The outline turns into the accent focus ring only when the group was reached by the keyboard, never after a click. Longer lists stay in menus.

Chrome shares one height, 2.25 rem: the title, tab, navigation and status bars, the collapsed sidebar's width, settings rows, menu entries (`style::MENU_ITEM_HEIGHT`, which menu height, scroll and submenu placement also read, with separators of 0.5 rem, `MENU_SEPARATOR_HEIGHT`) and the segmented control. Every horizontal separator between groups (menus, settings, properties, trash settings, forms, sidebar sections) is that 0.5 rem with its line centered: `elements::separator::horizontal`. Borders are not separators: a bar's edge, the line a scrolled list shows at its top, a resize handle or a Markdown rule keep their own drawing and take no such space. A new bar, row or entry takes that height unless it holds denser content. Lists are such content: file rows, column headers and the sidebar's folder tree are 1.75 rem (`ROW_HEIGHT`, `COLUMN_HEADER_HEIGHT`, `SIDEBAR_ROW_H`). The sidebar's places, WSL, drives and volumes are entries at 2.25 rem, in both sidebars, and its list keeps 0.5 rem above the first row and below the last (`sidebar::LIST_PADDING`).

Buttons of the bars framing a pane (navigation bar, quick search, status bar, Viewer bar, sidebar toolbar and rail) use `elements::bar_button`, as the title bar's actions: full-height cells as wide as the bar is high, 1 rem icons, the bar's edge holding the last cell, so the whole cell is the click target. Their fill spans the cell, square: `control_hover` under the pointer, `selected` with an accent icon for an active toggle. `SQUARE_FILL` in `bar_button.rs` switches the fill to a `rounded_md` shape inset 0.25 rem in the cell, as Adwaita's window buttons draw a disc in a full-height zone, with the same click target. Groups are divided by a 1 rem separator spaced 0.5 rem from their cells. A single choice in a bar (the status bar's details and grid) stays `elements::segmented`, which says only one option holds. The rail, 2.25 rem wide as every bar is high, stacks square cells.

Toggles use `elements::toggle::toggle`, including in trash settings: slightly tinted neutral track and thin outline when off, with a stronger knob; accent track and `text_on_accent` knob when on. Keep the knob's movement as an extra state cue.

| Need | Reference |
| --- | --- |
| Button | `elements/button/style.rs`: `button_style`, `ButtonVariant` |
| Bar button (navigation, status, Viewer) | `elements/bar_button.rs`: `cell`, `labelled`, `fill`, `icon` |
| Separator between groups | `elements/separator.rs`: `horizontal` |
| Single choice among a few options | `elements/segmented.rs`: `segmented`, `segment` |
| Form or modal | `components/modal.rs`: `Modal`, `form_row`, `action_button` |
| Operation failure notice | `app/dialog_windows/fixed/notice.rs`: heading, cause panel, kept locations, hint, optional Retry |
| Confirmation or notice | `components/confirmation.rs`: `body` (icon in a tinted disc, question, details, irreversibility warning), `cancel_button`, `confirm_button` with `Tone::Danger` for destructive actions |
| Secondary window and footer | `components/tool_window`; [window contracts](windows.md) |
| Plain field | `TextInput::new` with `TextInputOptions::default()` |
| Composite field | `input::field_frame` with `InputAppearance::Embedded` |

Views keep dimensions, identifiers and callbacks. For a disabled button, remove or guard its callback: style alone is not enough. Modals move by their title, with no dimmed background; their transparent layer blocks interaction behind them and reserves the status bar.

The standard field provides height 2.25 rem, text 0.8125 rem, line height 1.25, `surface` background, `border` then `accent` on focus, `rounded_md` radius and 0.75 rem horizontal margins. Do not recreate this frame per form. `clearable: true` explicitly enables Clear.

A composite field places the editor in `flex_1().min_w_0().h_full()` and commands in the same frame, without a second border. `Bare` leaves margins to the parent; `Inline` follows content geometry for rename and path. Do not hide suggestions with `overflow_hidden` in the integrated fallback.

## Preferences and responsiveness

`AppConfig` holds settings; globals apply theme and language immediately. The save queue writes in the background, in order. Keep old configurations readable with defaults; do not overwrite an unreadable configuration. Reset keeps session, geometry and duplicate preferences.

Rendering does no I/O or heavy computation. Reuse history/operation summaries and snapshots prepared by revision. Asynchronous responses must check their owner and revision before publishing. See [operations](operations.md) and [visual checks](../guides/visual-checks.md).

### Scrollbars

`components/scrollbar.rs` owns the shared rendering: permanent track background (`scrollbar_track`), thumb, hover/drag states and end margins. Do not hide the track at rest or add a per-list look. `TrackShape` only adapts the outline to the parent; select menus provide their inner radius via `select_menu`, and their track may be longer than the viewport without changing the content's scroll range. Keep event owners, identifiers and coordinate conversions.

For forms, modals, settings and notifications, `components/scroll_area.rs` composes these scrollbars with local state per surface and window. The calling view keeps the frame, constraints and viewport identifier. Thumbs reserve room only on overflow; both axes keep coordinates and captures in their owning window.

Secondary forms adapt their layout to available width in rems: `modal::responsive_form_row` stacks a label and its value if the view asks. Window footers may wrap. Zoom must not grow the native width to compensate for a rigid layout.

### Animations

An animation is never in the way of an action: clicks, navigation and drops stay immediate. Animate only elements appearing at the margin (fades, slides of a few pixels), in 150 ms at most, with exit shorter than entry and no bounce. Do not animate list content, selection, row hover, tab changes or resizing.

`elements/motion.rs` provides critically damped springs (`presence`, `glide`, `slide`) that redirect without a jump. Do not animate owned windows appearing (menus, tooltips): Windows overlays its own opening animation. Remove a repeating animation from rendering once it has no purpose.

A resize separator's accent fades in over its line while it is hovered or dragged (`elements/resize_highlight.rs`); the separator itself and the resizing stay immediate. Double-clicking a split's separator shares the two neighbouring panes equally; double-clicking the sidebar's edge fits it to its widest row, as rows truncate.

On Linux, an inactive window fades its title bar content to half opacity over 200 ms, as GNOME headers do (`components/backdrop.rs`). Open menus and modal prompts count as part of their owner, which keeps an active header while they hold the keyboard.

A computed folder size that changes signals the update with an accent background fading over 300 ms in the Size cell, an exception to the appearance duration. The initial calculation and identical results stay unanimated; an offscreen cell does not replay the flash when it appears.
