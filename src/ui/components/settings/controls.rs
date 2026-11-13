use crate::ui::components::tooltip::NativeTooltipExt;
use crate::ui::app::OxydeApp;
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::{OxydeTheme, ThemeContext};
use gpui::prelude::*;
use gpui::*;
use super::SettingsPicker;
use crate::ui::components::tooltip::Tooltip;
use crate::ui::i18n::FluentArgs;
use std::{cell::Cell, rc::Rc};

pub(super) fn icon(path: &'static str, theme: OxydeTheme) -> impl IntoElement {
    svg()
        .path(path)
        .size(rems(0.875))
        .flex_none()
        .text_color(theme.text)
}

/// A line between groups of rows.
pub(super) fn separator(theme: OxydeTheme) -> impl IntoElement {
    crate::ui::elements::separator::horizontal(theme.border)
}

pub(super) fn picker(
    view: &OxydeApp,
    handle: Entity<OxydeApp>,
    kind: SettingsPicker,
    id: &'static str,
    icon_path: &'static str,
    label: SharedString,
    theme: OxydeTheme,
) -> impl IntoElement {
    let active = if kind == SettingsPicker::About { view.settings_about_open }
        else { view.settings_picker == Some(kind) };
    let row_bounds = Rc::new(Cell::new(None::<Bounds<Pixels>>));
    let paint_bounds = row_bounds.clone();
    let paint_handle = handle.clone();
    let action = Rc::new(move |cx: &mut App| {
            handle.update(cx, |view, cx| {
                view.archive_format_bounds = None;
                if kind == SettingsPicker::About {
                    view.settings_about_open = !view.settings_about_open;
                    view.settings_picker = None;
                    view.settings_submenu_bounds = None;
                } else if view.settings_picker == Some(kind) {
                    view.settings_picker = None;
                    view.settings_submenu_bounds = None;
                } else {
                    view.settings_picker = Some(kind);
                    view.settings_submenu_bounds = row_bounds.get();
                }
                cx.notify();
            });
        });
    let key_action = action.clone(); let submenu_action = action.clone();
    let row = div()
        .id(id)
        .relative()
        .h(crate::ui::style::MENU_ITEM_HEIGHT)
        .px(rems(0.375))
        .flex()
        .items_center()
        .gap(rems(0.5))
        .rounded(rems(0.3125))
        .when(active, |row| row.bg(theme.selected))
        .hover(|s| s.bg(theme.menu_hover))
        .child(
            canvas(
                |bounds, _, _| bounds,
                move |_, bounds, window, cx| {
                    let bounds = crate::ui::components::popup::owner_bounds(window, cx, bounds);
                    paint_bounds.set(Some(bounds));
                    if active && kind != SettingsPicker::About {
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
        .child(icon(icon_path, theme))
        .child(div().flex_1().flex_shrink_0().whitespace_nowrap().child(label))
        .child(icon(if kind == SettingsPicker::About && active { "icons/chevron-left.svg" }
            else { "icons/chevron-right.svg" }, theme))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| action(cx));
    crate::ui::components::popup::keyboard::item(row, Rc::new(move |_, cx| key_action(cx)), Some(Rc::new(move |_, cx| submenu_action(cx))), None)

}

/// Row opening a separate window; the settings menu closes behind it.
pub(super) fn window_row(
    id: &'static str,
    icon_path: &'static str,
    label: SharedString,
    theme: OxydeTheme,
    on_open: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let open = Rc::new(on_open);
    let key_open = open.clone();
    let row = div()
        .id(id)
        .h(crate::ui::style::MENU_ITEM_HEIGHT)
        .px(rems(0.375))
        .flex()
        .items_center()
        .gap(rems(0.5))
        .rounded(rems(0.3125))
        .hover(|s| s.bg(theme.menu_hover))
        .child(icon(icon_path, theme))
        .child(div().flex_1().flex_shrink_0().whitespace_nowrap().child(label))
        .child(icon("icons/external-link.svg", theme))
        .on_mouse_down(MouseButton::Left, move |_, window, cx| open(window, cx));
    crate::ui::components::popup::keyboard::item(row, Rc::new(move |window, cx| key_open(window, cx)), None, None)
}

/// A row holding a stepper: one keyboard stop, Left and Right step, Enter restores the default.
fn stepper_row(
    id: &'static str,
    icon_path: &'static str,
    label: SharedString,
    stepper: crate::ui::elements::stepper::Stepper,
    on_step: crate::ui::elements::stepper::Step,
    on_reset: Rc<dyn Fn(&mut Window, &mut App)>,
    theme: OxydeTheme,
) -> crate::ui::components::popup::keyboard::KeyboardItem {
    let row = div().id(id).min_h(crate::ui::style::MENU_ITEM_HEIGHT).px(rems(0.375)).gap(rems(0.5)).flex().items_center()
        .rounded(rems(0.3125))
        .child(icon(icon_path, theme))
        .child(div().flex_1().flex_shrink_0().whitespace_nowrap().child(label))
        .child(stepper);
    crate::ui::components::popup::keyboard::item(row, on_reset, None, None).adjust(on_step)
}

pub(super) fn scale_row(view: &OxydeApp, handle: Entity<OxydeApp>, cx: &App) -> impl IntoElement {
    use crate::ui::scale::{BASE_REM_SIZE, MAX_REM_SIZE, MIN_REM_SIZE};
    let theme = *cx.theme();
    let reference = view.ui_rem_size();
    let mut args = FluentArgs::new();
    args.set("percentage", (reference * 100 + BASE_REM_SIZE / 2) / BASE_REM_SIZE);
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
    let stepper = crate::ui::elements::stepper::stepper("scale", cx.i18n().text_with_args(Message::SettingsScaleValue, &args),
            reference > MIN_REM_SIZE, reference < MAX_REM_SIZE, theme, on_step.clone())
        .reset(cx.i18n().text(Message::SettingsScaleReset), move |window, cx| reset(window, cx));
    stepper_row("settings-scale-row", "icons/type.svg", cx.i18n().text(Message::SettingsScale), stepper, on_step, on_reset, theme)
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
        .child(stepper_row("settings-search-limit-row", "icons/zap.svg", cx.i18n().text(Message::SettingsSearchLimit),
            stepper, on_step, on_reset, theme))
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

pub(super) fn choice(
    id: &'static str,
    label: SharedString,
    selected: bool,
    theme: OxydeTheme,
    on_select: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    choice_with_icon(id, label, selected, theme, None, on_select)
}

pub(super) fn submenu_choice(
    id: &'static str,
    label: SharedString,
    selected: bool,
    theme: OxydeTheme,
    on_select: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    let select = Rc::new(on_select);
    let key_select = select.clone();
    let row = crate::ui::components::select_menu::choice(div().id(id), theme, selected, true)
        .child(div().whitespace_nowrap().child(label))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| select(cx));
    crate::ui::components::popup::keyboard::item(row, Rc::new(move |_, cx| key_select(cx)), None, None)
}

pub(super) fn choice_with_icon(
    id: &'static str,
    label: SharedString,
    selected: bool,
    theme: OxydeTheme,
    icon_path: Option<&'static str>,
    on_select: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    let select = Rc::new(on_select);
    let key_select = select.clone();
    let row = crate::ui::components::select_menu::choice(div().id(id), theme, selected, true)
        .when_some(icon_path, |row, path| row.child(icon(path, theme)))
        .child(div().flex_1().child(label))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| select(cx));
    crate::ui::components::popup::keyboard::item(row, Rc::new(move |_, cx| key_select(cx)), None, None)
}

pub(super) fn settings_row(
    label: SharedString,
    description: SharedString,
    icon_path: &'static str,
    value: bool,
    id: &'static str,
    handle: Entity<OxydeApp>,
    theme: OxydeTheme,
    on_toggle: impl Fn(&mut OxydeApp, &mut Context<OxydeApp>) + 'static,
) -> impl IntoElement {
    let action = Rc::new(move |cx: &mut App| { handle.update(cx, |view, cx| on_toggle(view, cx)); });
    let key_action = action.clone();
    let row = div()
        .id(id)
        .min_h(crate::ui::style::MENU_ITEM_HEIGHT)
        .px(rems(0.375))
        .flex()
        .items_center()
        .gap(rems(0.5))
        .rounded(rems(0.3125))
        .hover(|s| s.bg(theme.control_hover))
        .fixed_tooltip(Tooltip::text(description))
        .child(icon(icon_path, theme))
        .child(div().flex_1().flex_shrink_0().whitespace_nowrap().text_size(rems(0.8125)).child(label))
        .child(
            crate::ui::elements::toggle::toggle("switch", value, theme),
        )
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            action(cx);
        });
    crate::ui::components::popup::keyboard::item(row, Rc::new(move |_, cx| key_action(cx)), None, None)
}
