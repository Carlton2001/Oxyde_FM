use crate::ui::components::popup::keyboard::{self, KeyboardItem};
use crate::ui::components::tooltip::{NativeTooltipExt, Tooltip};
use crate::ui::app::OxydeApp;
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::theme::{OxydeTheme, ThemeContext};
use gpui::prelude::*;
use gpui::*;
use super::SettingsPicker;
use std::{cell::Cell, rc::Rc};

/// Corner of every row, as the menus' entries.
const ROW_RADIUS: Rems = rems(0.3125);

pub(super) fn icon(path: &'static str, theme: OxydeTheme) -> impl IntoElement {
    svg()
        .path(path)
        .size(rems(0.875))
        .flex_none()
        .text_color(theme.text)
}

/// The frame shared by every row: its height, margins, icon gap and corner.
fn row(id: &'static str, icon_path: &'static str, theme: OxydeTheme) -> Stateful<Div> {
    div()
        .id(id)
        .min_h(crate::ui::style::MENU_ITEM_HEIGHT)
        .px(rems(0.375))
        .flex()
        .items_center()
        .gap(rems(0.5))
        .rounded(ROW_RADIUS)
        .child(icon(icon_path, theme))
}

/// The label, and under it a short hint for settings whose effect the name does not tell.
fn label(label: SharedString, hint: Option<SharedString>, theme: OxydeTheme) -> impl IntoElement {
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .when(hint.is_some(), |column| column.py(rems(0.375)))
        .child(div().whitespace_nowrap().child(label))
        .when_some(hint, |column, hint| {
            column.child(div().text_size(rems(0.75)).text_color(theme.text_muted).child(hint))
        })
}

/// A line between groups of rows.
pub(super) fn separator(theme: OxydeTheme) -> impl IntoElement {
    crate::ui::elements::separator::horizontal(theme.border)
}

pub(super) fn section(label: SharedString, theme: OxydeTheme) -> impl IntoElement {
    div()
        .py(rems(0.375))
        .px(rems(0.375))
        .text_size(rems(0.6875))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme.text_muted)
        .child(label.to_uppercase())
}

/// A row opening a submenu beside the panel. It opens on a click or once the pointer
/// rests on the row ([`hover`]); a click never closes it, as GTK's.
pub(super) fn picker(
    view: &OxydeApp,
    handle: Entity<OxydeApp>,
    kind: SettingsPicker,
    id: &'static str,
    icon_path: &'static str,
    text: SharedString,
    theme: OxydeTheme,
) -> impl IntoElement {
    use crate::ui::components::popup::hover;
    let active = view.settings_picker == Some(kind);
    let row_bounds = Rc::new(Cell::new(None::<Bounds<Pixels>>));
    let paint_bounds = row_bounds.clone();
    let paint_handle = handle.clone();
    let hover_handle = handle.clone();
    let open = Rc::new(move |cx: &mut App| {
        handle.update(cx, |view, cx| {
            if view.settings_picker != Some(kind) {
                view.close_settings_submenu();
                view.settings_picker = Some(kind);
                view.settings_submenu_bounds = row_bounds.get();
                cx.notify();
            }
        });
    });
    let (key_open, submenu_open, hover_open) = (open.clone(), open.clone(), open.clone());
    let row = row(id, icon_path, theme)
        .relative()
        .when(active, |row| row.bg(theme.selected))
        .hover(|s| s.bg(theme.menu_hover))
        .child(
            canvas(
                |bounds, _, _| bounds,
                move |_, bounds, window, cx| {
                    let bounds = crate::ui::components::popup::owner_bounds(window, cx, bounds);
                    paint_bounds.set(Some(bounds));
                    if active {
                        paint_handle.update(cx, |view, cx| {
                            if view.settings_submenu_bounds != Some(bounds) {
                                view.settings_submenu_bounds = Some(bounds);
                                cx.notify();
                            }
                        });
                    }
                },
            )
            .absolute()
            .left_0()
            .top_0()
            .size_full(),
        )
        .child(label(text, None, theme))
        .child(icon("icons/chevron-right.svg", theme))
        .on_mouse_move(move |event, window, cx| {
            cx.stop_propagation();
            hover::moved(event.position, cx);
            if hover_handle.read(cx).settings_picker == Some(kind) {
                hover::cancel(cx);
            } else {
                let open = hover_open.clone();
                hover::intend(format!("settings-open-{id}"), window, cx, move |_, cx| open(cx));
            }
        })
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            hover::cancel(cx);
            open(cx);
        });
    keyboard::item(row, Rc::new(move |_, cx| key_open(cx)), Some(Rc::new(move |_, cx| submenu_open(cx))), None)
}

/// Row opening a separate window; the settings menu closes behind it.
pub(super) fn window_row(
    id: &'static str,
    icon_path: &'static str,
    text: SharedString,
    theme: OxydeTheme,
    on_open: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let open = Rc::new(on_open);
    let key_open = open.clone();
    let row = row(id, icon_path, theme)
        .hover(|s| s.bg(theme.menu_hover))
        .child(label(text, None, theme))
        .child(icon("icons/external-link.svg", theme))
        .on_mouse_down(MouseButton::Left, move |_, window, cx| open(window, cx));
    keyboard::item(row, Rc::new(move |window, cx| key_open(window, cx)), None, None)
}

/// A row holding a stepper: one keyboard stop, Left and Right step, Enter restores the default.
fn stepper_row(
    id: &'static str,
    icon_path: &'static str,
    text: SharedString,
    hint: Option<SharedString>,
    stepper: crate::ui::elements::stepper::Stepper,
    on_step: crate::ui::elements::stepper::Step,
    on_reset: Rc<dyn Fn(&mut Window, &mut App)>,
    theme: OxydeTheme,
) -> KeyboardItem {
    let row = row(id, icon_path, theme)
        .child(label(text, hint, theme))
        .child(stepper);
    keyboard::item(row, on_reset, None, None).adjust(on_step)
}

pub(super) fn scale_row(view: &OxydeApp, handle: Entity<OxydeApp>, cx: &App) -> impl IntoElement {
    use crate::ui::scale::{BASE_REM_SIZE, MAX_REM_SIZE, MIN_REM_SIZE};
    let theme = *cx.theme();
    let reference = view.ui_rem_size();
    let reset_handle = handle.clone();
    let on_reset: Rc<dyn Fn(&mut Window, &mut App)> = Rc::new(move |window, cx| {
        crate::ui::components::popup::with_owner(window, cx, |window, cx| {
            reset_handle.update(cx, |view, cx| view.set_ui_rem_size(BASE_REM_SIZE, window, cx))
        })
    });
    let on_step: crate::ui::elements::stepper::Step = Rc::new(move |increase, window, cx| {
        crate::ui::components::popup::with_owner(window, cx, |window, cx| handle.update(cx, |view, cx| {
            let reference = view.ui_rem_size();
            let next = if increase { reference + 1 } else { reference.saturating_sub(1) };
            view.set_ui_rem_size(next, window, cx);
        }));
    });
    let reset = on_reset.clone();
    let stepper = crate::ui::elements::stepper::stepper("scale", scale_value(reference, cx),
            reference > MIN_REM_SIZE, reference < MAX_REM_SIZE, theme, on_step.clone())
        .reset(cx.i18n().text(Message::SettingsScaleReset), move |window, cx| reset(window, cx));
    stepper_row("settings-scale-row", "icons/type.svg", cx.i18n().text(Message::SettingsScale), None,
        stepper, on_step, on_reset, theme)
}

/// The interface scale shown for a reference rem size, as a percentage.
pub(super) fn scale_value(reference: u32, cx: &App) -> SharedString {
    use crate::ui::scale::BASE_REM_SIZE;
    let mut args = FluentArgs::new();
    args.set("percentage", (reference * 100 + BASE_REM_SIZE / 2) / BASE_REM_SIZE);
    cx.i18n().text_with_args(Message::SettingsScaleValue, &args)
}

pub(super) fn search_limit_row(view: &OxydeApp, handle: Entity<OxydeApp>, cx: &App) -> impl IntoElement {
    use oxyde_lib::models::preferences::{SEARCH_LIMIT_STEP, MAX_SEARCH_LIMIT};
    let theme = *cx.theme();
    let limit = view.search_limit();
    let reset_handle = handle.clone();
    let on_reset: Rc<dyn Fn(&mut Window, &mut App)> =
        Rc::new(move |_, cx| reset_handle.update(cx, |view, cx| view.reset_search_limit(cx)));
    let on_step: crate::ui::elements::stepper::Step =
        Rc::new(move |increase, _, cx| handle.update(cx, |view, cx| view.step_search_limit(increase, cx)));
    let reset = on_reset.clone();
    let stepper = crate::ui::elements::stepper::stepper("search-limit", limit.to_string(),
            limit > SEARCH_LIMIT_STEP, limit < MAX_SEARCH_LIMIT, theme, on_step.clone())
        .reset(cx.i18n().text(Message::SettingsSearchLimitReset), move |window, cx| reset(window, cx));
    div().id("settings-search-limit-tooltip")
        .fixed_tooltip(Tooltip::text(cx.i18n().text(Message::SettingsSearchLimitDescription)))
        .child(stepper_row("settings-search-limit-row", "icons/search.svg", cx.i18n().text(Message::SettingsSearchLimit),
            Some(cx.i18n().text(Message::SettingsSearchLimitHint)), stepper, on_step, on_reset, theme))
}

/// A choice of a list drawn in a submenu, its check in the icon column.
pub(super) fn choice(
    id: &'static str,
    text: SharedString,
    selected: bool,
    theme: OxydeTheme,
    on_select: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    let select = Rc::new(on_select);
    let key_select = select.clone();
    let row = crate::ui::components::select_menu::choice(div().id(id), theme, selected, true)
        .child(div().flex_1().whitespace_nowrap().child(text))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| select(cx));
    keyboard::item(row, Rc::new(move |_, cx| key_select(cx)), None, None)
}

/// A setting switched on or off, its description in a tooltip and, for the few whose
/// name does not tell the effect, a hint under the label.
pub(super) struct Toggle {
    pub(super) id: &'static str,
    pub(super) icon: &'static str,
    pub(super) label: Message,
    pub(super) description: Message,
    pub(super) hint: Option<Message>,
    pub(super) value: bool,
}

pub(super) fn toggle_row(
    toggle: Toggle,
    handle: Entity<OxydeApp>,
    cx: &App,
    on_toggle: impl Fn(&mut OxydeApp, &mut Context<OxydeApp>) + 'static,
) -> impl IntoElement {
    let theme = *cx.theme();
    let i18n = cx.i18n();
    let action = Rc::new(move |cx: &mut App| { handle.update(cx, |view, cx| on_toggle(view, cx)); });
    let key_action = action.clone();
    let row = row(toggle.id, toggle.icon, theme)
        .hover(|s| s.bg(theme.menu_hover))
        .fixed_tooltip(Tooltip::text(i18n.text(toggle.description)))
        .child(label(i18n.text(toggle.label), toggle.hint.map(|hint| i18n.text(hint)), theme))
        .child(crate::ui::elements::toggle::toggle("switch", toggle.value, theme))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| action(cx));
    keyboard::item(row, Rc::new(move |_, cx| key_action(cx)), None, None)
}
