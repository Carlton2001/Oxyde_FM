use crate::ui::app::{OxydeApp, PendingFileOperation};
use crate::ui::components::{
    icons::{get_icon_type, render_file_icon},
    input::TextInput,
    modal::{action_button, Modal},
};
use crate::ui::format::{date_format, format_date, format_number, format_size};
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::models::{ConflictAction, ConflictEntry, FileEntry};
use std::collections::{HashMap, HashSet};

#[derive(Clone)]
pub(crate) struct ConflictDialogState {
    advanced: bool,
    width: f32,
    height: f32,
    sidebar_width: f32,
    resize: Option<(Point<Pixels>, f32, f32, bool)>,
    focus: Option<FocusHandle>,
    selected: usize,
    filter: usize,
    query: String,
    input: Option<Entity<TextInput>>,
    collapsed: HashSet<String>,
    pub(crate) resolutions: HashMap<String, ConflictAction>,
    automatic: HashSet<String>,
    opposite_initialized: bool,
}

impl Default for ConflictDialogState {
    fn default() -> Self {
        Self {
            advanced: false,
            width: 52.0,
            height: 27.25,
            sidebar_width: 25.0,
            resize: None,
            focus: None,
            selected: 0,
            filter: 0,
            query: String::new(),
            input: None,
            collapsed: HashSet::new(),
            resolutions: HashMap::new(),
            automatic: HashSet::new(),
            opposite_initialized: false,
        }
    }
}
impl ConflictDialogState {
    fn record_choice(&mut self, conflicts: &[ConflictEntry], index: usize, action: ConflictAction) {
        let first = self.advanced && !self.opposite_initialized;
        if first {
            self.opposite_initialized = true;
            let opposite = match action {
                ConflictAction::Skip => ConflictAction::Replace,
                ConflictAction::Replace => ConflictAction::Skip,
            };
            let remaining: Vec<_> = conflicts
                .iter()
                .enumerate()
                .filter(|(i, c)| *i != index && decision(self, &c.source.path).is_none())
                .map(|(_, c)| c.source.path.clone())
                .collect();
            for path in remaining {
                self.resolutions.insert(path.clone(), opposite);
                self.automatic.insert(path);
            }
        }
        let selected = &conflicts[index];
        self.resolutions
            .insert(selected.source.path.clone(), action);
        self.automatic.remove(&selected.source.path);
        if selected.source.is_dir && !first {
            // Update automatic children, preserving explicit exceptions.
            for c in conflicts {
                if self.automatic.contains(&c.source.path)
                    && ancestors(&c.source.path).any(|a| a == selected.source.path)
                {
                    self.resolutions.insert(c.source.path.clone(), action);
                }
            }
        }
    }
    fn counts(&self, conflicts: &[ConflictEntry]) -> (usize, usize) {
        conflicts.iter().fold((0, 0), |(replace, skip), c| {
            match decision(self, &c.source.path) {
                Some(ConflictAction::Replace) => (replace + 1, skip),
                Some(ConflictAction::Skip) => (replace, skip + 1),
                None => (replace, skip),
            }
        })
    }
    /// The queue rechecks conflicts, so every known pair needs an explicit decision.
    /// Unanswered pairs are skipped; newly appearing conflicts still fail that check.
    pub(crate) fn completed_resolutions(
        &self,
        conflicts: &[ConflictEntry],
    ) -> HashMap<String, ConflictAction> {
        let mut resolutions = self.resolutions.clone();
        for conflict in conflicts {
            resolutions
                .entry(conflict.source.path.clone())
                .or_insert_with(|| {
                    decision(self, &conflict.source.path).unwrap_or(ConflictAction::Skip)
                });
        }
        resolutions
    }
}
fn restore_focus(handle: &Entity<OxydeApp>, window: &mut Window, cx: &mut App) {
    if handle.read(cx).pending_file_operations.is_empty() {
        let focus = handle.read(cx).focus_handle.clone();
        window.focus(&focus, cx);
    }
}
fn category(c: &ConflictEntry) -> usize {
    if c.source.modified == c.target.modified && c.source.size == c.target.size {
        1
    } else if c.source.modified > c.target.modified {
        2
    } else if c.source.modified < c.target.modified {
        3
    } else {
        0
    }
}
fn parent(path: &str) -> &str {
    path.rfind(['/', '\\']).map_or("", |i| &path[..=i])
}
fn ancestors(path: &str) -> impl Iterator<Item = &str> {
    let mut remaining = path;
    std::iter::from_fn(move || {
        let i = remaining.rfind(['/', '\\'])?;
        remaining = &remaining[..i];
        Some(remaining)
    })
}
fn decision(state: &ConflictDialogState, path: &str) -> Option<ConflictAction> {
    state
        .resolutions
        .get(path)
        .copied()
        .or_else(|| ancestors(path).find_map(|p| state.resolutions.get(p).copied()))
}
fn visible(p: &PendingFileOperation) -> Vec<usize> {
    filtered_indices(&p.conflicts, &p.dialog)
}
fn filtered_indices(conflicts: &[ConflictEntry], state: &ConflictDialogState) -> Vec<usize> {
    let query = state.query.to_lowercase();
    conflicts
        .iter()
        .enumerate()
        .filter(|(_, c)| {
            (state.filter == 0 || category(c) == state.filter)
                && c.name.to_lowercase().contains(&query)
                && !ancestors(&c.source.path).any(|a| state.collapsed.contains(a))
        })
        .map(|(i, _)| i)
        .collect()
}
fn edit(handle: &Entity<OxydeApp>, cx: &mut App, update: impl FnOnce(&mut PendingFileOperation)) {
    handle.update(cx, |app, cx| {
        if let Some(p) = app.pending_file_operations.front_mut() {
            update(p);
        }
        cx.notify();
    });
}
fn choose(handle: &Entity<OxydeApp>, action: ConflictAction, all: bool, cx: &mut App) {
    handle.update(cx, |app, cx| {
        let Some(p) = app.pending_file_operations.front_mut() else {
            return;
        };
        let indices = if all {
            if p.dialog.advanced {
                visible(p)
            } else {
                (0..p.conflicts.len()).collect()
            }
        } else {
            vec![p.dialog.selected]
        };
        for i in indices {
            p.dialog.record_choice(&p.conflicts, i, action);
        }
        let visible_indices: HashSet<_> = visible(p).into_iter().collect();
        let next = (p.dialog.selected + 1..p.conflicts.len())
            .chain(0..p.dialog.selected)
            .find(|&i| {
                (decision(&p.dialog, &p.conflicts[i].source.path).is_none()
                    || (p.dialog.advanced
                        && p.dialog.automatic.contains(&p.conflicts[i].source.path)))
                    && (!p.dialog.advanced || visible_indices.contains(&i))
            });
        if let Some(i) = next {
            p.dialog.selected = i;
        }
        if !p.dialog.advanced && (all || next.is_none()) {
            app.apply_conflict_decisions(cx);
        }
        cx.notify();
    });
}
fn button(
    id: &'static str,
    message: Message,
    primary: bool,
    handle: Entity<OxydeApp>,
    callback: impl Fn(&Entity<OxydeApp>, &mut App) + 'static,
    cx: &App,
) -> Stateful<Div> {
    button_with_label(id, message, None, primary, handle, callback, cx)
}
fn button_with_label(
    id: &'static str,
    message: Message,
    label: Option<SharedString>,
    primary: bool,
    handle: Entity<OxydeApp>,
    callback: impl Fn(&Entity<OxydeApp>, &mut App) + 'static,
    cx: &App,
) -> Stateful<Div> {
    let t = *cx.theme();
    let icon = match message {
        Message::ConflictSkip => Some("icons/ban.svg"),
        Message::ConflictReplace => Some("icons/check.svg"),
        Message::ConflictAdvanced => Some("icons/settings-2.svg"),
        _ => None,
    };
    let callback = std::rc::Rc::new(callback);
    let click_callback = callback.clone();
    let keyboard_handle = handle.clone();
    action_button(id)
        .tab_index(0)
        .flex_none()
        .whitespace_nowrap()
        .rounded_sm()
        .border(rems(0.0625))
        .border_color(gpui::transparent_black())
        .focus(|d| d.border_color(t.accent))
        .flex()
        .items_center()
        .gap(rems(0.375))
        .bg(if primary { t.accent } else { t.stepper_surface })
        .text_color(if primary { t.text_on_accent } else { t.text })
        .hover(|d| d.opacity(0.85))
        .when_some(icon, |d, path| {
            d.child(
                svg()
                    .path(path)
                    .size(rems(0.875))
                    .flex_none()
                    .text_color(if primary { t.text_on_accent } else { t.text }),
            )
        })
        .child(label.unwrap_or_else(|| cx.i18n().text(message)))
        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
            click_callback(&handle, cx);
            restore_focus(&handle, window, cx);
        })
        .on_key_down(move |event, window, cx| {
            if event.keystroke.key == "enter" || event.keystroke.key == "space" {
                cx.stop_propagation();
                callback(&keyboard_handle, cx);
                restore_focus(&keyboard_handle, window, cx);
            }
        })
}
fn automatic_badge(cx: &App) -> impl IntoElement {
    div()
        .flex_none()
        .whitespace_nowrap()
        .px(rems(0.25))
        .rounded_sm()
        .border(rems(0.0625))
        .border_color(cx.theme().border)
        .text_size(rems(0.625))
        .text_color(cx.theme().text_muted)
        .child(cx.i18n().text(Message::ConflictAutomatic))
}
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
fn conflict_icon(entry: &FileEntry, size: Rems, system: bool, large: bool, cx: &App) -> AnyElement {
    let icon = get_icon_type(&entry.name, entry.is_dir, Some(&entry.path), false, false);
    if !system && icon == crate::ui::components::icons::IconType::FileVideo {
        svg()
            .path("icons/conflict-file-video.svg")
            .size(size)
            .flex_none()
            .text_color(cx.theme().icon_video)
            .into_any_element()
    } else {
        div()
            .size(size)
            .flex_none()
            .child(render_file_icon(
                entry.path.clone(),
                icon,
                size,
                if entry.is_dir {
                    cx.theme().accent
                } else {
                    cx.theme().icon_muted
                },
                system,
                large,
            ))
            .into_any_element()
    }
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
                .text_color(t.toast_success)
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
            t.background.blend(t.accent.opacity(0.04))
        } else {
            t.background.blend(t.header.opacity(0.8))
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
pub(crate) fn render_conflict_dialog(
    pending_state: &mut PendingFileOperation,
    system: bool,
    window: &mut Window,
    handle: Entity<OxydeApp>,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let t = *cx.theme();
    let pending = pending_state;
    if pending.dialog.focus.is_none() {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        pending.dialog.focus = Some(focus);
    }
    if pending.dialog.advanced && pending.dialog.input.is_none() {
        let h = handle.clone();
        let escape = handle.clone();
        let input = cx.new(|cx| {
            TextInput::new(
                cx,
                "",
                String::new(),
                move |text, _, cx| {
                    edit(&h, cx, |p| {
                        p.dialog.query = text.to_string();
                        let rows = visible(p);
                        if !rows.contains(&p.dialog.selected) {
                            if let Some(&i) = rows.first() {
                                p.dialog.selected = i;
                            }
                        }
                    })
                },
                |_, _| {},
                move |window, cx| {
                    escape.update(cx, |a, cx| a.cancel_pending_file_operation(cx));
                    restore_focus(&escape, window, cx);
                },
                |_, _, _, _, _| {},
                false,
                true,
                false,
                None,
                window,
            )
        });
        pending.dialog.input = Some(input.clone());
    }
    let advanced = pending.dialog.advanced;
    let max_width = (window.viewport_size().width / window.rem_size()).max(2.0) - 2.0;
    let width = pending.dialog.width.min(max_width);
    let sidebar_width = pending.dialog.sidebar_width.min((width - 20.0).max(10.0));
    let sidebar_drag = handle.clone();
    let corner_drag = handle.clone();
    let rows = visible(&pending);
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
            .gap(rems(1.0))
            .mt(rems(0.5))
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
    let mut body = div()
        .flex()
        .flex_col()
        .gap(rems(if advanced { 0.0 } else { 0.75 }));
    if advanced {
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
                div()
                    .relative()
                    .w(rems(15.0))
                    .h(rems(2.25))
                    .flex_none()
                    .rounded(rems(0.375))
                    .bg(t.surface)
                    .border(rems(0.0625))
                    .border_color(t.border)
                    .child(div().h_full().pr(rems(1.5)).child(input))
                    .child(
                        div().absolute().right(rems(0.75)).top(rems(0.625)).child(
                            svg()
                                .path("icons/search.svg")
                                .size(rems(0.875))
                                .flex_none()
                                .text_color(t.text_muted),
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
                                .text_color(t.text_muted),
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
                .h(rems(pending.dialog.height))
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
                        .w(rems(0.1875))
                        .h_full()
                        .flex_none()
                        .bg(t.border)
                        .hover(|d| d.bg(t.accent))
                        .on_mouse_down(MouseButton::Left, move |event, _, cx| {
                            cx.stop_propagation();
                            edit(&sidebar_drag, cx, |p| {
                                p.dialog.resize =
                                    Some((event.position, p.dialog.sidebar_width, 0.0, true))
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
    } else {
        let mut args = FluentArgs::new();
        args.set("name", current.name.clone());
        body = body
            .child(
                div()
                    .text_size(rems(0.9375))
                    .child(cx.i18n().text_with_args(Message::ConflictPrompt, &args)),
            )
            .child(comparison);
    }
    let title = if advanced {
        cx.i18n().text(Message::ConflictAdvancedTitle).to_string()
    } else {
        let mut args = FluentArgs::new();
        args.set(
            "current",
            format_number((pending.dialog.selected + 1) as u64, cx.i18n().language()),
        );
        args.set(
            "total",
            format_number(pending.conflicts.len() as u64, cx.i18n().language()),
        );
        cx.i18n()
            .text_with_args(Message::ConflictStepTitle, &args)
            .to_string()
    };
    let cancel = handle.clone();
    let header = div()
        .flex()
        .items_center()
        .gap(rems(0.5))
        .w_full()
        .child(
            svg()
                .path("icons/copy-x.svg")
                .size(rems(1.0))
                .text_color(t.text_muted),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .whitespace_nowrap()
                .text_size(rems(0.875))
                .font_weight(FontWeight::MEDIUM)
                .child(title.clone()),
        )
        .when(!advanced, |d| {
            d.child(
                div()
                    .flex_none()
                    .whitespace_nowrap()
                    .mr(rems(1.0))
                    .text_size(rems(0.65))
                    .font_weight(FontWeight::BOLD)
                    .text_color(t.text_muted.opacity(0.7))
                    .child(format!(
                        "{} : {} {}",
                        cx.i18n()
                            .text(if pending.is_move {
                                Message::OpMove
                            } else {
                                Message::Copy
                            })
                            .to_uppercase(),
                        format_number(pending.paths.len() as u64, cx.i18n().language()),
                        cx.i18n().text(Message::ConflictItems).to_uppercase()
                    )),
            )
        })
        .child(
            div()
                .id("conflict-close")
                .p(rems(0.25))
                .hover(|d| d.bg(t.control_hover))
                .child(
                    svg()
                        .path("icons/x.svg")
                        .size(rems(1.0))
                        .text_color(t.text_muted),
                )
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    cx.stop_propagation();
                    cancel.update(cx, |a, cx| a.cancel_pending_file_operation(cx));
                    restore_focus(&cancel, window, cx);
                }),
        );
    let mut modal = Modal::new("conflict-dialog", title, body)
        .width(rems(if advanced { width } else { 42.0 }))
        .header(header)
        .header_background(t.surface)
        .body_background(t.background)
        .body_padding(
            rems(if advanced { 0.0 } else { 1.25 }),
            rems(if advanced { 0.0 } else { 1.25 }),
        )
        .wrap_actions();
    if advanced {
        let all_resolved = pending
            .conflicts
            .iter()
            .all(|c| decision(&pending.dialog, &c.source.path).is_some());
        let (replace, skip) = pending.dialog.counts(&pending.conflicts);
        let mut args = FluentArgs::new();
        args.set("replace_count", replace);
        args.set("skip_count", skip);
        args.set(
            "replace_number",
            format_number(replace as u64, cx.i18n().language()),
        );
        args.set(
            "skip_number",
            format_number(skip as u64, cx.i18n().language()),
        );
        let apply_label = cx
            .i18n()
            .text_with_args(Message::ConflictApplySummary, &args);
        modal = modal
            .action(button(
                "conflict-back",
                Message::ConflictBack,
                false,
                handle.clone(),
                |h, cx| edit(h, cx, |p| p.dialog.advanced = false),
                cx,
            ))
            .action(div().flex_1())
            .action(
                button_with_label(
                    "conflict-apply",
                    Message::ConflictApply,
                    Some(apply_label),
                    all_resolved,
                    handle.clone(),
                    move |h, cx| {
                        if all_resolved {
                            h.update(cx, |a, cx| a.apply_conflict_decisions(cx));
                        }
                    },
                    cx,
                )
                .when(!all_resolved, |d| d.opacity(0.5)),
            );
    } else {
        modal = modal
            .action(button(
                "conflict-cancel",
                Message::Cancel,
                false,
                handle.clone(),
                |h, cx| h.update(cx, |a, cx| a.cancel_pending_file_operation(cx)),
                cx,
            ))
            .action(div().flex_1())
            .action(button(
                "conflict-advanced",
                Message::ConflictAdvanced,
                false,
                handle.clone(),
                |h, cx| edit(h, cx, |p| p.dialog.advanced = true),
                cx,
            ))
            .action(div().flex_1())
            .action(button(
                "conflict-skip-all",
                Message::ConflictSkipAll,
                false,
                handle.clone(),
                |h, cx| choose(h, ConflictAction::Skip, true, cx),
                cx,
            ))
            .action(button(
                "conflict-replace-all",
                Message::ConflictReplaceAll,
                true,
                handle.clone(),
                |h, cx| choose(h, ConflictAction::Replace, true, cx),
                cx,
            ));
    }
    if advanced {
        modal = modal.overlay(
            div()
                .absolute()
                .bottom(rems(0.125))
                .right(rems(0.125))
                .id("conflict-resize")
                .size(rems(0.875))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    svg()
                        .path("icons/grip.svg")
                        .size(rems(0.75))
                        .flex_none()
                        .text_color(t.text_muted.opacity(0.5)),
                )
                .on_mouse_down(MouseButton::Left, move |event, _, cx| {
                    cx.stop_propagation();
                    edit(&corner_drag, cx, |p| {
                        p.dialog.resize =
                            Some((event.position, p.dialog.width, p.dialog.height, false));
                    });
                }),
        );
    }
    let resize_handle = handle.clone();
    let release_handle = handle.clone();
    let resize_events = canvas(
        |_, _, _| (),
        move |_, _, window, _| {
            let moved = resize_handle.clone();
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                if phase != DispatchPhase::Capture {
                    return;
                }
                let Some((start, initial_width, initial_height, sidebar)) = moved
                    .read(cx)
                    .pending_file_operations
                    .front()
                    .and_then(|p| p.dialog.resize)
                else {
                    return;
                };
                let delta = event.position - start;
                let dx = delta.x / window.rem_size();
                let dy = delta.y / window.rem_size();
                let max_w = (window.viewport_size().width / window.rem_size() - 2.0).max(20.0);
                let max_h = (window.viewport_size().height / window.rem_size() - 13.0).max(10.0);
                edit(&moved, cx, |p| {
                    if !event.dragging() {
                        p.dialog.resize = None;
                        return;
                    }
                    if sidebar {
                        p.dialog.sidebar_width =
                            (initial_width + dx).clamp(10.0, (p.dialog.width - 20.0).max(10.0));
                    } else {
                        p.dialog.width =
                            (initial_width + 2.0 * dx).clamp(47.5_f32.min(max_w), max_w);
                        p.dialog.height =
                            (initial_height + 2.0 * dy).clamp(18.0_f32.min(max_h), max_h);
                    }
                });
            });
            let released = release_handle.clone();
            window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                if phase == DispatchPhase::Capture
                    && event.button == MouseButton::Left
                    && released
                        .read(cx)
                        .pending_file_operations
                        .front()
                        .is_some_and(|p| p.dialog.resize.is_some())
                {
                    edit(&released, cx, |p| p.dialog.resize = None);
                }
            });
        },
    )
    .absolute()
    .size_full();
    div()
        .absolute()
        .inset_0()
        .track_focus(pending.dialog.focus.as_ref().unwrap())
        .on_key_down(move |event, window, cx| {
            if event.keystroke.key != "tab" {
                cx.stop_propagation();
            }
            if event.keystroke.key == "escape" {
                handle.update(cx, |a, cx| a.cancel_pending_file_operation(cx));
                restore_focus(&handle, window, cx);
            }
        })
        .on_action(|_: &crate::ui::app::Ouvrir, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Copier, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::CopierNom, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::CopierChemin, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Couper, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Coller, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Renommer, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Supprimer, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::ToutSelectionner, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Annuler, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Refaire, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Actualiser, _, cx| cx.stop_propagation())
        .child(resize_events)
        .child(modal)
}

#[cfg(test)]
mod tests {
    use super::{category, decision, filtered_indices, ConflictDialogState};
    use oxyde_lib::models::{ConflictAction, ConflictEntry, FileEntry};
    fn conflict(path: &str, source_time: u64, target_time: u64, size: u64) -> ConflictEntry {
        ConflictEntry {
            name: path.rsplit('/').next().unwrap().into(),
            source: FileEntry {
                path: path.into(),
                modified: source_time,
                size,
                ..Default::default()
            },
            target: FileEntry {
                modified: target_time,
                size,
                ..Default::default()
            },
        }
    }
    #[test]
    fn categories_compare_source_to_target_and_require_equal_sizes() {
        assert_eq!(category(&conflict("a", 2, 1, 10)), 2);
        assert_eq!(category(&conflict("a", 1, 2, 10)), 3);
        assert_eq!(category(&conflict("a", 1, 1, 10)), 1);
        let mut c = conflict("a", 1, 1, 10);
        c.target.size = 20;
        assert_eq!(category(&c), 0);
    }
    #[test]
    fn closest_parent_decision_is_inherited_and_explicit_choice_wins() {
        let mut state = ConflictDialogState::default();
        state
            .resolutions
            .insert("C:\\folder".into(), ConflictAction::Skip);
        assert_eq!(
            decision(&state, "C:\\folder\\sub\\file"),
            Some(ConflictAction::Skip)
        );
        assert_eq!(decision(&state, "C:\\folder-other\\file"), None);
        state
            .resolutions
            .insert("C:\\folder\\sub".into(), ConflictAction::Replace);
        assert_eq!(
            decision(&state, "C:\\folder\\sub\\file"),
            Some(ConflictAction::Replace)
        );
        state
            .resolutions
            .insert("C:\\folder\\sub\\file".into(), ConflictAction::Skip);
        assert_eq!(
            decision(&state, "C:\\folder\\sub\\file"),
            Some(ConflictAction::Skip)
        );
    }
    #[test]
    fn filtered_rows_keep_original_indices_and_collapsed_descendants_are_hidden() {
        let conflicts = vec![
            conflict("root/older", 1, 2, 10),
            conflict("root/newer", 2, 1, 10),
        ];
        let mut state = ConflictDialogState::default();
        state.filter = 2;
        assert_eq!(filtered_indices(&conflicts, &state), vec![1]);
        state.query = "NEWER".into();
        assert_eq!(filtered_indices(&conflicts, &state), vec![1]);
        state.collapsed.insert("root".into());
        assert!(filtered_indices(&conflicts, &state).is_empty());
    }
    #[test]
    fn applying_partial_decisions_skips_unanswered_pairs_without_losing_inheritance() {
        let conflicts = vec![
            conflict("root", 1, 2, 10),
            conflict("root/child", 1, 2, 10),
            conflict("other", 1, 2, 10),
            conflict("root/explicit", 1, 2, 10),
        ];
        let mut state = ConflictDialogState::default();
        state
            .resolutions
            .insert("root".into(), ConflictAction::Replace);
        state
            .resolutions
            .insert("root/explicit".into(), ConflictAction::Skip);
        let choices = state.completed_resolutions(&conflicts);
        assert_eq!(choices["root"], ConflictAction::Replace);
        assert_eq!(choices["root/child"], ConflictAction::Replace);
        assert_eq!(choices["root/explicit"], ConflictAction::Skip);
        assert_eq!(choices["other"], ConflictAction::Skip);
        assert!(!choices.contains_key("new-conflict"));
    }
    #[test]
    fn first_advanced_choice_assigns_the_opposite_to_every_other_conflict() {
        let conflicts = vec![
            conflict("root/a", 2, 1, 10),
            conflict("root/b", 1, 2, 10),
            conflict("root/c", 1, 1, 10),
        ];
        for (first, opposite) in [
            (ConflictAction::Skip, ConflictAction::Replace),
            (ConflictAction::Replace, ConflictAction::Skip),
        ] {
            let mut state = ConflictDialogState {
                advanced: true,
                filter: 2,
                query: "a".into(),
                ..Default::default()
            };
            state.record_choice(&conflicts, 0, first);
            assert_eq!(decision(&state, "root/a"), Some(first));
            assert_eq!(decision(&state, "root/b"), Some(opposite));
            assert_eq!(decision(&state, "root/c"), Some(opposite));
            assert!(!state.automatic.contains("root/a"));
            assert!(state.automatic.contains("root/b"));
            assert!(state.automatic.contains("root/c"));
            assert_eq!(
                state.counts(&conflicts),
                if first == ConflictAction::Skip {
                    (2, 1)
                } else {
                    (1, 2)
                }
            );
        }
    }
    #[test]
    fn later_choices_override_defaults_without_reinitializing_other_decisions() {
        let conflicts = vec![
            conflict("a", 1, 2, 10),
            conflict("b", 1, 2, 10),
            conflict("c", 1, 2, 10),
        ];
        let mut state = ConflictDialogState {
            advanced: true,
            ..Default::default()
        };
        state.record_choice(&conflicts, 0, ConflictAction::Skip);
        state.record_choice(&conflicts, 1, ConflictAction::Skip);
        state.record_choice(&conflicts, 0, ConflictAction::Replace);
        assert_eq!(state.counts(&conflicts), (2, 1));
        assert!(!state.automatic.contains("a"));
        assert!(!state.automatic.contains("b"));
        assert!(state.automatic.contains("c"));
        state.advanced = false;
        state.advanced = true;
        state.record_choice(&conflicts, 1, ConflictAction::Replace);
        assert_eq!(state.counts(&conflicts), (3, 0));
        assert!(state.automatic.contains("c"));
    }
    #[test]
    fn simple_choices_remain_sequential_and_existing_choices_are_preserved() {
        let conflicts = vec![
            conflict("a", 1, 2, 10),
            conflict("b", 1, 2, 10),
            conflict("c", 1, 2, 10),
        ];
        let mut state = ConflictDialogState::default();
        state.record_choice(&conflicts, 0, ConflictAction::Skip);
        assert!(decision(&state, "b").is_none());
        assert!(state.automatic.is_empty());
        state.advanced = true;
        state.record_choice(&conflicts, 1, ConflictAction::Skip);
        assert_eq!(decision(&state, "a"), Some(ConflictAction::Skip));
        assert_eq!(decision(&state, "c"), Some(ConflictAction::Replace));
        assert!(!state.automatic.contains("a"));
    }
    #[test]
    fn folder_choices_update_automatic_children_and_keep_explicit_exceptions() {
        let mut conflicts = vec![
            conflict("root", 1, 2, 10),
            conflict("root/child", 1, 2, 10),
            conflict("root/exception", 1, 2, 10),
        ];
        conflicts[0].source.is_dir = true;
        let mut state = ConflictDialogState {
            advanced: true,
            ..Default::default()
        };
        state.record_choice(&conflicts, 0, ConflictAction::Skip);
        assert_eq!(
            decision(&state, "root/child"),
            Some(ConflictAction::Replace)
        );
        state.record_choice(&conflicts, 2, ConflictAction::Skip);
        state.record_choice(&conflicts, 0, ConflictAction::Replace);
        assert_eq!(
            decision(&state, "root/child"),
            Some(ConflictAction::Replace)
        );
        assert_eq!(
            decision(&state, "root/exception"),
            Some(ConflictAction::Skip)
        );
        state.record_choice(&conflicts, 0, ConflictAction::Skip);
        assert_eq!(decision(&state, "root/child"), Some(ConflictAction::Skip));
        assert_eq!(
            decision(&state, "root/exception"),
            Some(ConflictAction::Skip)
        );
    }
}
