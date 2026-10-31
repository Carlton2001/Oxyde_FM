# Interface conventions

Business state and actions belong to `src/ui/app/`. Components receive data and callbacks and own only their local focus, scroll or expansion state. `src/ui/elements/` provides primitives. The [development guide](../guides/development.md) covers splitting.

## Themes and dimensions

- Get the theme through `cx.theme()` and use `OxydeTheme` tokens. Add a token to every palette if a shared role is missing; keep local values for component-specific effects.
- The thirteen themes are declared by `ThemeId::ALL`. File colors are centralized in `components/icons/colors.rs` and applied by `render_file_icon`.
- Express dimensions in `rems()`, initial reference 16 logical pixels. Use `ui::scale::rem_pixels` for a `Pixels` API, `logical` for older values at scale 16. These conversions stay on the UI thread.
- Coordinates from GPUI are already in pixels: do not convert twice. Content zoom does not resize the native window.
- The default weight is Regular (400), from `ui/style.rs`. Separately rendered roots apply it too. Another weight must be an intentional variation.

## Languages and formats

`cx.i18n()` resolves `Message` variants. For any user text, add or reuse a key and its translations in `locales/{fr,en,de,es,it}.ftl`, then check catalogs and parameters.

Keep whole sentences in Fluent, with numeric parameters for plurals. Paths and user names stay literal data; strip bidirectional isolates only if the result becomes a system path. Persistent errors keep their key so they follow a language change. Technical details stay in logs.

`ui/format.rs` centralizes numbers, sizes and dates. Language and the persisted date format are separate from the regional system locale. Messages fall back to French; a formatting failure is logged.

## Reusing components

Checkboxes use `elements::checkbox::checkbox`: 1 rem square, 1/16 rem outline, 1/4 rem radius. At rest the neutral outline mixes surface and secondary text, staying less present than labels. Checked uses an accent background and outline with a centered check. Partial keeps the accent outline but reduces the accent background to 20% over the neutral surface, with a text-colored dash. Views keep click areas, focus and actions. Toggles and plain choice marks keep their distinct roles.

Toggles use `elements::toggle::toggle`, including in trash settings: slightly tinted neutral track and thin outline when off, with a stronger knob; accent track and `text_on_accent` knob when on. Keep the knob's movement as an extra state cue.

| Need | Reference |
| --- | --- |
| Button | `elements/button/style.rs`: `button_style`, `ButtonVariant` |
| Form or modal | `components/modal.rs`: `Modal`, `form_row`, `action_button` |
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

A computed folder size that changes signals the update with an accent background fading over 300 ms in the Size cell, an exception to the appearance duration. The initial calculation and identical results stay unanimated; an offscreen cell does not replay the flash when it appears.
