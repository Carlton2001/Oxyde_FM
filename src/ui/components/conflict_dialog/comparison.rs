use super::controls::{automatic_badge, button, choose};
use super::rules::{ancestors, decision, parent};
use crate::ui::app::{OxydeApp, PendingFileOperation};
use crate::ui::components::icons::{entry_icon, render_file_icon};
use crate::ui::format::{date_format, format_date, format_number, format_size};
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::models::{ConflictAction, FileEntry};

pub(super) fn conflict_icon(
    entry: &FileEntry,
    size: Rems,
    system: bool,
    large: bool,
    cx: &App,
) -> AnyElement {
    let icon = entry_icon(entry);
    div()
        .size(size)
        .flex_none()
        .child(render_file_icon(
            entry.path.clone(),
            icon,
            size, cx.theme(),
            system,
            large,
        ))
        .into_any_element()
}
fn metadata(label: SharedString, value: String, highlight: bool, cx: &App) -> impl IntoElement {
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(rems(0.5))
        .whitespace_nowrap()
        .child(
            div()
                .text_color(cx.theme().text_muted)
                .child(format!("{label} :")),
        )
        .child(
            div()
                .text_color(if highlight {
                    cx.theme().accent
                } else {
                    cx.theme().text
                })
                .child(value),
        )
}
fn card(
    entry: &FileEntry,
    other: &FileEntry,
    source: bool,
    system: bool,
    cx: &App,
) -> impl IntoElement {
    let t = *cx.theme();
    let newer = entry.modified > other.modified;
    let warning = newer && entry.size < other.size;
    let badge_color = if warning { t.error } else { t.accent };
    let mut badges = div().flex().flex_none().items_center().gap(rems(0.5));
    if newer {
        badges = badges.child(
            div()
                .flex_none()
                .whitespace_nowrap()
                .rounded_full()
                .px(rems(0.5))
                .text_size(rems(0.625))
                .font_weight(FontWeight::SEMIBOLD)
                .border(rems(0.0625))
                .border_color(badge_color.opacity(0.3))
                .bg(badge_color.opacity(0.15))
                .text_color(badge_color)
                .child(cx.i18n().text(Message::ConflictNewer).to_uppercase()),
        );
    }
    if entry.size > other.size {
        badges = badges.child(
            div()
                .flex_none()
                .whitespace_nowrap()
                .rounded_full()
                .px(rems(0.5))
                .text_size(rems(0.625))
                .text_color(t.success_text)
                .bg(t.toast_success.opacity(0.15))
                .child(cx.i18n().text(Message::ConflictLarger).to_uppercase()),
        );
    }
    let mut size = format_size(entry.size, entry.is_dir, true, cx.i18n().language());
    if entry.is_dir && (entry.folders_count.is_some() || entry.files_count.is_some()) {
        size.push_str(&format!(
            " ({} {})",
            format_number(
                entry.folders_count.unwrap_or(0) + entry.files_count.unwrap_or(0),
                cx.i18n().language()
            ),
            cx.i18n().text(Message::ConflictItems)
        ));
    }
    div()
        .flex()
        .flex_col()
        .flex_none()
        .gap(rems(0.75))
        .p(rems(1.0))
        .rounded(rems(0.75))
        .bg(if newer {
            t.active_background.blend(t.accent.opacity(0.04))
        } else {
            t.active_background.blend(t.header.opacity(0.8))
        })
        .border(rems(0.0625))
        .border_color(if newer { t.accent } else { t.border })
        .when(newer, |d| d.shadow(crate::ui::style::rem_shadow(false)))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .min_h(rems(1.25))
                .gap(rems(0.5))
                .text_size(rems(0.6875))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(if newer {
                    t.accent.opacity(0.8)
                } else {
                    t.text_muted.opacity(0.75)
                })
                .child(
                    div().whitespace_nowrap().child(
                        cx.i18n()
                            .text(if source {
                                Message::ConflictSource
                            } else {
                                Message::ConflictTarget
                            })
                            .to_uppercase(),
                    ),
                )
                .child(badges),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(rems(1.25))
                .child(
                    div()
                        .size(rems(3.0))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(conflict_icon(entry, rems(2.5), system, true, cx)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(rems(0.25))
                        .child(
                            div()
                                .text_size(rems(0.875))
                                .font_weight(FontWeight::MEDIUM)
                                .text_ellipsis()
                                .child(entry.name.clone()),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .gap_x(rems(1.5))
                                .gap_y(rems(0.125))
                                .child(metadata(
                                    cx.i18n().text(Message::ColumnSize),
                                    size,
                                    entry.size > other.size,
                                    cx,
                                ))
                                .child(metadata(
                                    cx.i18n().text(Message::ColumnModified),
                                    format_date(entry.modified, date_format(cx)),
                                    newer,
                                    cx,
                                )),
                        )
                        .child(
                            div()
                                .text_size(rems(0.75))
                                .text_color(t.text_muted.opacity(0.6))
                                .text_ellipsis()
                                .child(parent(&entry.path).to_string()),
                        ),
                ),
        )
}
pub(super) fn render_comparison(
    pending: &PendingFileOperation,
    system: bool,
    handle: &Entity<OxydeApp>,
    cx: &App,
) -> Div {
    let t = *cx.theme();
    let current = &pending.conflicts[pending.dialog.selected];
    let mut comparison = div()
        .flex()
        .flex_col()
        .gap(rems(0.25))
        .child(card(&current.source, &current.target, true, system, cx))
        .child(
            div()
                .text_center()
                .text_size(rems(0.625))
                .text_color(t.text_muted)
                .child("VS"),
        )
        .child(card(&current.target, &current.source, false, system, cx));
    if !pending
        .dialog
        .resolutions
        .contains_key(&current.source.path)
    {
        if let Some(inherited) =
            ancestors(&current.source.path).find(|p| pending.dialog.resolutions.contains_key(*p))
        {
            let action = pending.dialog.resolutions[inherited];
            let mut args = FluentArgs::new();
            args.set("parent", inherited);
            args.set(
                "action",
                cx.i18n()
                    .text(if action == ConflictAction::Replace {
                        Message::ConflictReplace
                    } else {
                        Message::ConflictSkip
                    })
                    .to_string(),
            );
            comparison = comparison.child(
                div()
                    .text_size(rems(0.75))
                    .text_color(t.text_muted)
                    .child(cx.i18n().text_with_args(Message::ConflictInherited, &args)),
            );
        }
    }
    if let Some(action) = decision(&pending.dialog, &current.source.path) {
        comparison = comparison.child(
            div()
                .flex()
                .items_center()
                .gap(rems(0.5))
                .when(
                    pending.dialog.automatic.contains(&current.source.path),
                    |d| d.child(automatic_badge(cx)),
                )
                .px(rems(0.75)).py(rems(0.5))
                .rounded_md().border(rems(0.0625)).border_color(t.border)
                .bg(t.selected).font_weight(FontWeight::SEMIBOLD)
                .text_color(t.accent)
                .child(cx.i18n().text(if action == ConflictAction::Replace {
                    Message::ConflictReplace
                } else {
                    Message::ConflictSkip
                })),
        );
    }
    comparison = comparison.child(
        div()
            .flex()
            .justify_center()
            .gap(rems(0.75))
            .p(rems(0.75)).mt(rems(0.5))
            .rounded_md().border(rems(0.0625)).border_color(t.border)
            .bg(t.surface)
            .child(button(
                "conflict-skip",
                Message::ConflictSkip,
                false,
                handle.clone(),
                |h, cx| choose(h, ConflictAction::Skip, false, cx),
                cx,
            ))
            .child(button(
                "conflict-replace",
                Message::ConflictReplace,
                true,
                handle.clone(),
                |h, cx| choose(h, ConflictAction::Replace, false, cx),
                cx,
            )),
    );
    comparison
}
