use crate::ui::app::{GroupSelection, OxydeApp};
use crate::ui::components::tooltip::{NativeTooltipExt, Tooltip};
use crate::ui::date_groups::GroupKey;
use crate::ui::format::format_number;
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::theme::OxydeTheme;
use gpui::{prelude::*, *};

pub(super) fn render(
    group: GroupKey,
    count: usize,
    collapsed: bool,
    selection: GroupSelection,
    pane_id: String,
    handle: Entity<OxydeApp>,
    height: f32,
    cx: &mut App,
) -> AnyElement {
    let theme = *OxydeTheme::global(cx);
    let checkbox = selection_checkbox(&group, selection, &pane_id, handle.clone(), theme, cx);
    let toggle_group = group.clone();
    let label_id = SharedString::from(format!("group-label-{pane_id}-{group:?}"));
    let mut args = FluentArgs::new();
    args.set("count", count);
    args.set("number", format_number(count as u64, cx.i18n().language()));
    div()
        .id(SharedString::from(format!(
            "date-group-{pane_id}-{group:?}"
        )))
        .flex_none()
        .h(px(height))
        .w_full()
        .flex()
        .items_center()
        .gap(rems(0.5))
        .px(rems(0.5))
        .text_size(rems(0.8125))
        .text_color(theme.text)
        .border_b(rems(0.0625))
        .border_color(theme.border)
        .hover(|style| style.bg(theme.control_hover))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            cx.stop_propagation();
            handle.update(cx, |view, cx| {
                view.finish_pointer_drag(cx);
                view.activate_pane(&pane_id, cx);
                if let Some(pane) = view.state.find_pane_mut(&pane_id) {
                    let groups = &mut pane.active_tab_mut().collapsed_groups;
                    if !groups.remove(&toggle_group) {
                        groups.insert(toggle_group.clone());
                    }
                }
                cx.notify();
            });
        })
        .child(checkbox)
        .child(
            svg()
                .path(if collapsed {
                    "icons/chevron-right.svg"
                } else {
                    "icons/chevron-down.svg"
                })
                .size(rems(1.0))
                .text_color(theme.accent),
        )
        .child(
            div()
                .id(label_id)
                .min_w_0()
                .truncate()
                .native_tooltip(Tooltip::wrapped_text(group.label(cx.i18n())))
                .child(group.label(cx.i18n())),
        )
        .child(
            div()
                .flex_none()
                .rounded_sm()
                .border(rems(0.0625))
                .border_color(theme.border)
                .px(rems(0.375))
                .text_size(rems(0.75))
                .text_color(theme.text_muted)
                .child(cx.i18n().text_with_args(Message::DateGroupCount, &args)),
        )
        .into_any_element()
}

fn selection_checkbox(
    group: &GroupKey,
    selection: GroupSelection,
    pane_id: &str,
    handle: Entity<OxydeApp>,
    theme: OxydeTheme,
    cx: &mut App,
) -> impl IntoElement {
    let tooltip = cx.i18n().text(if selection == GroupSelection::All {
        Message::GroupDeselectAll
    } else {
        Message::GroupSelectAll
    });
    let id = SharedString::from(format!("group-selection-{pane_id}-{group:?}"));
    let group = group.clone();
    let pane_id = pane_id.to_string();
    div()
        .id(id)
        .size(rems(1.5))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .native_tooltip(Tooltip::wrapped_text(tooltip))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(move |_, window, cx| {
            cx.stop_propagation();
            handle.update(cx, |view, cx| {
                view.focus_handle.focus(window, cx);
                view.toggle_group_selection(&pane_id, &group, cx);
            });
        })
        .child(
            crate::ui::elements::checkbox::checkbox(theme, selection == GroupSelection::All, selection == GroupSelection::Partial),
        )
}
