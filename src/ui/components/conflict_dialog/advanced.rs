use super::comparison::conflict_icon;
use super::controls::{automatic_badge, edit, visible};
use super::rules::{ancestors, category, decision, parent};
use crate::ui::app::{OxydeApp, PendingFileOperation};
use crate::ui::components::modal::action_button;
use crate::ui::format::format_number;
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::models::ConflictAction;

fn status_dot(category: usize, cx: &App) -> impl IntoElement {
    let t = *cx.theme();
    div()
        .size(rems(0.5))
        .flex_none()
        .rounded_full()
        .bg(match category {
            2 => t.accent,
            3 => t.error,
            1 => gpui::transparent_black(),
            _ => t.text_muted.opacity(0.3),
        })
        .when(category == 1, |d| {
            d.border(rems(0.0625))
                .border_color(t.text_muted.opacity(0.5))
        })
}
pub(super) fn render_advanced_body(
    pending: &PendingFileOperation,
    system: bool,
    sidebar_width: f32,
    rows: Vec<usize>,
    comparison: Div,
    handle: &Entity<OxydeApp>,
    window: &Window,
    cx: &mut Context<OxydeApp>,
) -> Div {
    let t = *cx.theme();
    let sidebar_drag = handle.clone();
    let mut body = div().flex().flex_col().flex_1().min_h_0().gap(rems(0.0));
    let mut tabs = div()
        .flex()
        .flex_none()
        .items_center()
        .gap(rems(0.25))
        .px(rems(1.25))
        .h(rems(2.625))
        .bg(t.dialog_tabs)
        .border_b(rems(0.0625))
        .border_color(t.border);
    for (filter, message) in [
        Message::ConflictAll,
        Message::ConflictIdentical,
        Message::ConflictRecentTab,
        Message::ConflictOlderTab,
    ]
    .into_iter()
    .enumerate()
    {
        let count = pending
            .conflicts
            .iter()
            .filter(|c| filter == 0 || category(c) == filter)
            .count();
        let h = handle.clone();
        tabs = tabs.child(
            action_button(("conflict-filter", filter))
                .flex()
                .flex_none()
                .items_center()
                .gap(rems(0.75))
                .h_full()
                .rounded_none()
                .whitespace_nowrap()
                .px(rems(1.0))
                .border_b(rems(0.0625))
                .border_color(if pending.dialog.filter == filter {
                    t.accent
                } else {
                    gpui::transparent_black()
                })
                .text_color(if pending.dialog.filter == filter {
                    t.accent
                } else if count == 0 {
                    t.icon_disabled
                } else {
                    t.text_muted
                })
                .child(cx.i18n().text(message))
                .when(count > 0, |d| {
                    d.child(
                        div()
                            .text_size(rems(0.75))
                            .opacity(0.7)
                            .child(format_number(count as u64, cx.i18n().language())),
                    )
                })
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    if count == 0 {
                        return;
                    }
                    edit(&h, cx, |p| {
                        p.dialog.filter = filter;
                        if let Some(&i) = visible(p).first() {
                            p.dialog.selected = i;
                        }
                    });
                }),
        );
    }
    if let Some(input) = pending.dialog.input.clone() {
        input.update(cx, |input, cx| {
            input.set_placeholder(cx.i18n().text(Message::ConflictSearch), cx)
        });
        tabs = tabs.child(div().flex_1()).child(
            crate::ui::components::input::field_frame(&input, window, cx)
                .relative()
                .w(rems(15.0))
                .flex_none()
                .child(div().flex_1().min_w_0().h_full().pr(rems(1.5)).child(input))
                .child(
                    div().absolute().right(rems(0.75)).top(rems(0.625)).child(
                        svg()
                            .path("icons/search.svg")
                            .size(rems(0.875))
                            .flex_none()
                            .text_color(t.text),
                    ),
                ),
        );
    }
    let mut list = div()
        .id("conflict-list")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .py(rems(0.5))
        .pl(rems(1.25));
    let depth = pending
        .conflicts
        .iter()
        .map(|c| ancestors(&c.source.path).count())
        .min()
        .unwrap_or(0);
    for &i in &rows {
        let c = &pending.conflicts[i];
        let h = handle.clone();
        let collapse = handle.clone();
        let path = c.source.path.clone();
        let resolved = decision(&pending.dialog, &path);
        let mut row = div()
            .id(("conflict-row", i))
            .flex()
            .items_center()
            .gap(rems(0.75))
            .py(rems(0.5))
            .pl(rems(
                0.5 + ancestors(&path).count().saturating_sub(depth) as f32,
            ))
            .pr(rems(0.75))
            .border_l(rems(0.1875))
            .border_color(if i == pending.dialog.selected {
                t.accent
            } else {
                gpui::transparent_black()
            })
            .bg(if i == pending.dialog.selected {
                t.accent.opacity(0.08)
            } else {
                gpui::transparent_black()
            })
            .hover(|d| d.bg(t.hover))
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                edit(&h, cx, |p| p.dialog.selected = i)
            });
        if c.source.is_dir {
            row = row.child(
                div()
                    .id(("conflict-fold", i))
                    .size(rems(1.25))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        svg()
                            .path(if pending.dialog.collapsed.contains(&path) {
                                "icons/chevron-right.svg"
                            } else {
                                "icons/chevron-down.svg"
                            })
                            .size(rems(0.875))
                            .flex_none()
                            .text_color(t.text),
                    )
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        cx.stop_propagation();
                        edit(&collapse, cx, |p| {
                            if !p.dialog.collapsed.remove(&path) {
                                p.dialog.collapsed.insert(path.clone());
                            }
                            let rows = visible(p);
                            if !rows.contains(&p.dialog.selected) {
                                if let Some(&i) = rows.first() {
                                    p.dialog.selected = i;
                                }
                            }
                        });
                    }),
            );
        }
        row = row
            .when(!c.source.is_dir, |r| {
                r.child(div().w(rems(1.25)).flex_none())
            })
            .child(conflict_icon(&c.source, rems(1.0), system, false, cx))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(div().text_ellipsis().child(c.name.clone()))
                    .child(
                        div()
                            .text_size(rems(0.75))
                            .text_color(t.text_muted)
                            .text_ellipsis()
                            .child(parent(&c.source.path).to_string()),
                    ),
            );
        let mut status = div().flex().flex_none().items_center().gap(rems(0.5));
        if let Some(action) = resolved {
            status = status.child(
                svg()
                    .path(if action == ConflictAction::Replace {
                        "icons/circle-check.svg"
                    } else {
                        "icons/circle-x.svg"
                    })
                    .size(rems(0.75))
                    .flex_none()
                    .text_color(if action == ConflictAction::Replace {
                        t.toast_success
                    } else {
                        t.error
                    })
                    .opacity(
                        if pending.dialog.resolutions.contains_key(&c.source.path)
                            && !pending.dialog.automatic.contains(&c.source.path)
                        {
                            1.0
                        } else {
                            0.45
                        },
                    ),
            );
        }
        row = row.child(
            status
                .when(pending.dialog.automatic.contains(&c.source.path), |d| {
                    d.child(automatic_badge(cx))
                })
                .child(status_dot(category(c), cx)),
        );
        list = list.child(row);
    }
    let legend = div()
        .flex()
        .flex_none()
        .items_center()
        .justify_between()
        .gap(rems(0.75))
        .px(rems(0.75))
        .py(rems(0.5))
        .border_t(rems(0.0625))
        .border_color(t.border)
        .text_size(rems(0.75))
        .text_color(t.text_muted)
        .children(
            [
                (2, Message::ConflictNewer),
                (3, Message::ConflictOlder),
                (1, Message::ConflictIdentical),
            ]
            .into_iter()
            .map(|(category, message)| {
                div()
                    .flex()
                    .items_center()
                    .gap(rems(0.375))
                    .whitespace_nowrap()
                    .child(status_dot(category, cx))
                    .child(cx.i18n().text(message))
            }),
        );
    body = body.child(tabs).child(
        div()
            .relative()
            .flex()
            .flex_1()
            .min_h_0()
            .child(
                div()
                    .w(rems(sidebar_width))
                    .flex_none()
                    .flex()
                    .flex_col()
                    .bg(t.surface)
                    .border_r(rems(0.0625))
                    .border_color(t.border)
                    .child(list)
                    .child(legend),
            )
            .child(
                div()
                    .id("conflict-sidebar-resize")
                    .cursor(CursorStyle::ResizeLeftRight)
                    .w(rems(0.1875))
                    .h_full()
                    .flex_none()
                    .bg(t.border)
                    .hover(|d| d.bg(t.accent))
                    .on_mouse_down(MouseButton::Left, move |event, _, cx| {
                        cx.stop_propagation();
                        edit(&sidebar_drag, cx, |p| {
                            p.dialog.sidebar_resize =
                                Some((event.position, p.dialog.sidebar_width))
                        });
                    }),
            )
            .child(
                div()
                    .id("conflict-detail")
                    .flex_1()
                    .min_w_0()
                    .overflow_y_scroll()
                    .p(rems(1.25))
                    .child(if rows.is_empty() {
                        div()
                            .text_color(t.text_muted)
                            .child(cx.i18n().text(Message::ConflictEmpty))
                            .into_any_element()
                    } else {
                        comparison.into_any_element()
                    }),
            ),
    );
    body
}
