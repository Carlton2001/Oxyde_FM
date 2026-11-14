//! What the column filters hide, in both views: a chip in the status bar and a way out of
//! a list they empty.
use crate::ui::app::OxydeApp;
use crate::ui::components::dialog::button;
use crate::ui::elements::button::ButtonVariant;
use crate::ui::components::tooltip::{NativeTooltipExt, Tooltip};
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::theme::OxydeTheme;
use crate::ui::workspace::TabState;
use gpui::prelude::*;
use gpui::*;

/// « 12 hidden » and its clear button while a column filter is set, its tooltip listing them.
pub(super) fn chip(tab: &TabState, pane_id: &str, handle: Entity<OxydeApp>, theme: OxydeTheme, cx: &App) -> Option<impl IntoElement> {
    if !tab.has_column_filters() {
        return None;
    }
    let hidden = tab.files.len().saturating_sub(tab.filtered_files().len());
    let mut args = FluentArgs::new();
    args.set("count", hidden);
    args.set("number", crate::ui::format::format_number(hidden as u64, cx.i18n().language()));
    let summary = crate::ui::components::file_list::filters_summary(tab, cx.i18n(), crate::ui::format::date_format(cx));
    let pane_id = pane_id.to_string();
    Some(
        div()
            .id(SharedString::from(format!("filter-chip-{pane_id}")))
            .flex_none()
            .h(rems(1.5))
            .pl(rems(0.5))
            .flex()
            .items_center()
            .gap(rems(0.375))
            .rounded(rems(0.3125))
            .bg(theme.selected)
            .text_color(theme.accent)
            .fixed_tooltip(Tooltip::text(summary))
            .child(svg().path("icons/funnel.svg").size(rems(0.875)).flex_none().text_color(theme.accent))
            .child(cx.i18n().text_with_args(Message::FilterHidden, &args))
            .child(
                div()
                    .id(SharedString::from(format!("filter-chip-clear-{pane_id}")))
                    .size(rems(1.5))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(rems(0.3125))
                    .hover(move |style| style.bg(theme.control_hover))
                    .child(svg().path("icons/x.svg").size(rems(0.875)).text_color(theme.accent))
                    .fixed_tooltip(Tooltip::text(cx.i18n().text(Message::FilterClearAll)))
                    .on_click(move |_, _, cx| handle.update(cx, |this, cx| this.clear_column_filters(&pane_id, cx))),
            ),
    )
}

/// The empty list's message, with « Clear filters » when the column filters emptied it.
pub(super) fn empty_message(message: String, tab: &TabState, pane_id: &str, handle: Entity<OxydeApp>, cx: &App) -> AnyElement {
    if tab.files.is_empty() || !tab.filtered_files().is_empty() || !tab.has_column_filters() {
        return div().child(message).into_any_element();
    }
    let pane_id = pane_id.to_string();
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(rems(0.75))
        .child(message)
        .child(
            button(SharedString::from(format!("filter-empty-clear-{pane_id}")), cx.i18n().text(Message::FilterClearAll), ButtonVariant::Secondary)
                .on_press(move |_, cx| handle.update(cx, |this, cx| this.clear_column_filters(&pane_id, cx))),
        )
        .into_any_element()
}
