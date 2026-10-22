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

pub(super) fn separator(theme: OxydeTheme) -> impl IntoElement {
    div()
        .h(rems(0.0625))
        .my(rems(0.25))
        .bg(theme.border)
        .flex_none()
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
        .h(rems(2.0))
        .px(rems(0.375))
        .flex()
        .items_center()
        .gap(rems(0.5))
        .rounded_md()
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
            .size_full(),
        )
        .child(icon(icon_path, theme))
        .child(div().flex_1().flex_shrink_0().whitespace_nowrap().child(label))
        .child(icon(if kind == SettingsPicker::About && active { "icons/chevron-left.svg" }
            else { "icons/chevron-right.svg" }, theme))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| action(cx));
    crate::ui::components::popup::keyboard::item(row, Rc::new(move |_, cx| key_action(cx)), Some(Rc::new(move |_, cx| submenu_action(cx))), None)

}

pub(super) fn scale_row(view: &OxydeApp, handle: Entity<OxydeApp>, cx: &App) -> impl IntoElement {
    use crate::ui::scale::{BASE_REM_SIZE, MAX_REM_SIZE, MIN_REM_SIZE};
    let theme = *cx.theme();
    let reference = view.ui_rem_size();
    let mut args = FluentArgs::new();
    args.set("percentage", (reference * 100 + BASE_REM_SIZE / 2) / BASE_REM_SIZE);
    let reset = handle.clone();
    let value = div().id("scale-reset").min_w(rems(3.25)).h(rems(1.5)).flex().items_center().justify_center()
        .rounded_md()
        .hover(|style| style.bg(theme.control_hover))
        .native_tooltip(Tooltip::text(cx.i18n().text(Message::SettingsScaleReset)))
        .child(cx.i18n().text_with_args(Message::SettingsScaleValue, &args))
        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
            cx.stop_propagation();
            crate::ui::components::popup::with_owner(window, cx, |window, cx| reset.update(cx, |view, cx| view.set_ui_rem_size(BASE_REM_SIZE, window, cx)));
        });
    div().min_h(rems(2.0)).px(rems(0.375)).gap(rems(0.5)).flex().items_center()
        .child(icon("icons/type.svg", theme))
        .child(div().flex_1().flex_shrink_0().whitespace_nowrap().child(cx.i18n().text(Message::SettingsScale)))
        .child(crate::ui::elements::stepper::stepper(
            "scale", value, reference > MIN_REM_SIZE, reference < MAX_REM_SIZE, theme,
            move |increase, window, cx| {
                crate::ui::components::popup::with_owner(window, cx, |window, cx| handle.update(cx, |view, cx| {
                    let reference = view.ui_rem_size();
                    let next = if increase { reference + 1 } else { reference.saturating_sub(1) };
                    view.set_ui_rem_size(next, window, cx);
                }));
            },
        ))
}

pub(super) fn search_limit_row(view: &OxydeApp, handle: Entity<OxydeApp>, cx: &App) -> impl IntoElement {
    use oxyde_lib::models::preferences::{SEARCH_LIMIT_STEP, MAX_SEARCH_LIMIT};
    let theme = *cx.theme();
    let limit = view.search_limit();
    let reset = handle.clone();
    let value = div().id("search-limit-reset").min_w(rems(3.25)).h(rems(1.5))
        .flex().items_center().justify_center()
        .rounded_md()
        .hover(|style| style.bg(theme.control_hover))
        .native_tooltip(Tooltip::text(cx.i18n().text(Message::SettingsSearchLimitReset)))
        .child(limit.to_string())
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            cx.stop_propagation();
            reset.update(cx, |view, cx| view.reset_search_limit(cx));
        });
    div().id("settings-search-limit-row").min_h(rems(2.0)).px(rems(0.375)).gap(rems(0.5)).flex().items_center()
        .native_tooltip(Tooltip::text(cx.i18n().text(Message::SettingsSearchLimitDescription)))
        .child(icon("icons/zap.svg", theme))
        .child(div().flex_1().flex_shrink_0().whitespace_nowrap().child(cx.i18n().text(Message::SettingsSearchLimit)))
        .child(crate::ui::elements::stepper::stepper(
            "search-limit", value, limit > SEARCH_LIMIT_STEP, limit < MAX_SEARCH_LIMIT, theme,
            move |increase, _, cx| {
                handle.update(cx, |view, cx| view.step_search_limit(increase, cx));
            },
        ))
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
    let row = div().id(id)
        .h(rems(1.75)).flex_none().px(rems(0.5))
        .flex().items_center().gap(rems(0.5))
        .rounded(rems(0.3125))
        .hover(|s| s.bg(theme.menu_hover))
        .child(div().size(rems(1.0)).flex_none().when(selected, |slot| {
            slot.child(svg().path("icons/check.svg").size(rems(1.0)).text_color(theme.text))
        }))
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
    let row = crate::ui::components::select_menu::row(div().id(id), theme, selected)
        .when_some(icon_path, |row, path| row.child(icon(path, theme)))
        .child(div().flex_1().child(label))
        .when(selected, |row| row.child(icon("icons/check.svg", theme)))
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
        .min_h(rems(2.0))
        .px(rems(0.375))
        .flex()
        .items_center()
        .gap(rems(0.5))
        .rounded_md()
        .hover(|s| s.bg(theme.control_hover))
        .native_tooltip(Tooltip::text(description))
        .child(icon(icon_path, theme))
        .child(div().flex_1().flex_shrink_0().whitespace_nowrap().text_size(rems(0.8125)).child(label))
        .child(
            crate::ui::elements::toggle::toggle(value, theme),
        )
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            action(cx);
        });
    crate::ui::components::popup::keyboard::item(row, Rc::new(move |_, cx| key_action(cx)), None, None)
}
