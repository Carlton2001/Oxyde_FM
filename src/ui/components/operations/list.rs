//! The operations beside the selected one, as the Duplicates window lists its
//! locations: running ones, then finished ones, each under a section title and in a
//! framed box.
use super::detail::{kind, source_summary, status_badge, status_label};
use super::items::outcome::badge;
use super::{is_final, OperationsPanelView};
use crate::ui::components::scrollbar::{render_scrollbar_with_pointer, ScrollbarAxis};
use crate::ui::elements::resize_highlight::resize_highlight;
use crate::ui::scale::rem_pixels;
use crate::ui::elements::progress_bar::progress_bar;
use crate::ui::format::format_number;
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::style::TabularFigures;
use crate::ui::theme::{OxydeTheme, ThemeContext};
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::systems::file_ops::{OpStatus, OperationProgress, OperationSummary};

/// As wide as the Duplicates window's sources.
pub(super) const WIDTH: Rems = rems(17.5);
/// The narrowest list, and the room it leaves the detail at least.
const MIN_WIDTH: Rems = rems(12.0);
const MIN_DETAIL: Rems = rems(28.0);
/// Corners of a section's box.
const BOX_RADIUS: Rems = rems(0.375);

pub(super) fn fraction(progress: &OperationProgress) -> f32 {
    let ratio = if progress.total_bytes > 0 {
        progress.processed_bytes as f64 / progress.total_bytes as f64
    } else if progress.total_files > 0 {
        progress.processed_files as f64 / progress.total_files as f64
    } else if progress.status == OpStatus::Completed {
        1.0
    } else {
        0.0
    };
    ratio.clamp(0.0, 1.0) as f32
}

impl OperationsPanelView {
    /// A section title, as the Duplicates window's: icon, name in capitals, count.
    fn render_section(&self, icon: &'static str, message: Message, count: usize, first: bool, theme: OxydeTheme, cx: &mut Context<Self>) -> Div {
        div()
            .flex_none()
            .when(!first, |title| title.mt(rems(1.25)))
            .mb(rems(0.5))
            .flex()
            .items_center()
            .gap(rems(0.5))
            .text_size(rems(0.75))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(theme.text_muted)
            .child(svg().path(icon).size(rems(0.875)).flex_none().text_color(theme.text))
            .child(cx.i18n().text(message).to_uppercase())
            .child(div().tabular_figures().child(format_number(count as u64, cx.i18n().language())))
    }

    /// An operation: its kind and progress or state, its sources, and a bar while it
    /// runs. Selected as a Duplicates location: the selection fill, the name in accent.
    fn render_operation_row(&self, summary: &OperationSummary, first: bool, last: bool, theme: OxydeTheme, cx: &mut Context<Self>) -> AnyElement {
        let progress = &summary.progress;
        let id = progress.id.clone();
        let selected = self.selected.as_deref() == Some(id.as_str());
        let (kind, icon) = kind(&progress.op_type);
        let running = !is_final(&progress.status);
        let fraction = fraction(progress);
        let finalizing = progress.status == OpStatus::Running && fraction >= 1.0;
        let paused = progress.status == OpStatus::Paused;
        let color = if paused { theme.text_muted } else { theme.accent };
        // Running: how far it got; else its state, on its badge.
        let state = if running && !paused {
            div()
                .flex_none()
                .text_size(rems(0.75))
                .font_weight(FontWeight::MEDIUM)
                .tabular_figures()
                .text_color(theme.text_muted)
                .child(format!("{}%", (fraction * 100.0).round() as u32))
        } else {
            let (fill, text) = status_badge(&progress.status, theme);
            badge(cx.i18n().text(status_label(&progress.status)), fill, text)
        };
        let summary_text = source_summary(summary.first_source.as_deref(), summary.source_count, cx);
        let chosen = id.clone();
        div()
            .id(SharedString::from(format!("operation-row-{id}")))
            .flex_none()
            .when(first, |row| row.rounded_t(BOX_RADIUS))
            .when(last, |row| row.rounded_b(BOX_RADIUS))
            .when(!last, |row| row.border_b(rems(0.0625)).border_color(theme.border))
            .px(rems(0.75))
            .py(rems(0.625))
            .flex()
            .items_center()
            .gap(rems(0.75))
            .bg(if selected { theme.selected } else { theme.active_background })
            .hover(move |style| style.bg(if selected { theme.selected } else { theme.hover }))
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, window, cx| {
                window.focus(&this.focus, cx);
                this.selected = Some(chosen.clone());
                cx.notify();
            }))
            .child(svg().path(icon).size(rems(0.875)).flex_none().text_color(theme.text))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(rems(0.2))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(rems(0.5))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(if selected { theme.accent } else { theme.text })
                                    .child(cx.i18n().text(kind)),
                            )
                            .child(state),
                    )
                    .when_some(summary_text, |column, text| {
                        column.child(div().truncate().text_size(rems(0.75)).text_color(theme.text_muted).child(text))
                    })
                    .when(running, |column| {
                        column.child(
                            div().pt(rems(0.25)).child(
                                progress_bar(format!("operation-row-bar-{id}"), fraction, color, finalizing, cx).h(rems(0.1875)),
                            ),
                        )
                    }),
            )
            .into_any_element()
    }

    /// The sections and their scrollbar.
    pub(super) fn render_list(&self, summaries: &[OperationSummary], width: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.theme();
        let (running, finished): (Vec<_>, Vec<_>) = summaries.iter().rev().partition(|summary| !is_final(&summary.progress.status));
        let mut list = div()
            .id("operations-list")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.list_scroll)
            .px(rems(1.25))
            .py(rems(0.8))
            .flex()
            .flex_col();
        let sections = [
            ("icons/play.svg", Message::OpsGroupRunning, &running),
            ("icons/circle-check.svg", Message::OpsGroupFinished, &finished),
        ];
        let mut first = true;
        for (icon, message, group) in sections {
            if group.is_empty() {
                continue;
            }
            list = list.child(self.render_section(icon, message, group.len(), first, theme, cx));
            first = false;
            let count = group.len();
            let rows = group
                .iter()
                .enumerate()
                .map(|(index, summary)| self.render_operation_row(summary, index == 0, index + 1 == count, theme, cx))
                .collect::<Vec<_>>();
            list = list.child(
                div()
                    .flex_none()
                    .flex()
                    .flex_col()
                    .rounded(BOX_RADIUS)
                    .border(rems(0.0625))
                    .border_color(theme.border)
                    .children(rows),
            );
        }
        let scroll = self.list_scroll.clone();
        let viewport = scroll.bounds().size.height.as_f32();
        let content = viewport + scroll.max_offset().y.as_f32();
        let entity_id = cx.entity().entity_id();
        let dragging = self.scroll_drag.as_ref().is_some_and(|(track, _, _)| track.ends_with(super::view::LIST_TRACK));
        div()
            .relative()
            .w(px(width))
            .flex_none()
            .h_full()
            .child(list)
            .child(
                div().absolute().top_0().bottom_0().right_0().w(px(crate::ui::components::scrollbar::track_width()))
                    .child(render_scrollbar_with_pointer(theme, cx.entity(), ScrollbarAxis::Vertical, viewport.max(1.0), content.max(1.0),
                        -scroll.offset().y.as_f32(), dragging, super::view::LIST_TRACK.into(), None, scroll.clone(),
                        move |this: &mut Self, track, viewport, content, _, cx| {
                            this.scroll_drag = Some((track, viewport, content));
                            cx.notify(entity_id);
                        })),
            )
    }

    /// The list's width now: as dragged, within what the window leaves the detail.
    pub(super) fn list_display_width(&self, window: &Window) -> f32 {
        let default = rem_pixels(WIDTH).as_f32();
        let max = (window.viewport_size().width.as_f32() - rem_pixels(MIN_DETAIL).as_f32()).max(rem_pixels(MIN_WIDTH).as_f32());
        self.list_width
            .map_or(default, |width| width * self.rem_px)
            .clamp(rem_pixels(MIN_WIDTH).as_f32(), max)
    }

    /// Keeps the width in the preferences once a drag or a double click settles it.
    fn save_list_width(&self, cx: &mut Context<Self>) {
        let width = self.list_width;
        let _ = self.app.update(cx, |app, cx| app.set_operations_list_width(width, cx));
    }

    pub(super) fn finish_list_resize(&mut self, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.resizing_list) {
            self.save_list_width(cx);
        }
    }

    /// The edge between the list and the detail, a copy of the file chooser's sidebar
    /// edge (itself the main window's): dragged to resize, a double click restores
    /// the default width.
    pub(super) fn render_list_edge(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.theme();
        let width = self.list_display_width(window);
        let start = move |this: &mut Self, event: &MouseDownEvent, cx: &mut Context<Self>| {
            if event.click_count == 2 {
                this.list_width = None;
                this.save_list_width(cx);
                cx.notify();
                return;
            }
            this.resizing_list = true;
            this.grab_offset = event.position.x.as_f32() - width;
            cx.notify();
        };
        let highlighted = self.list_edge_hovered || self.resizing_list;
        div()
            .id("operations-list-edge")
            .relative()
            .w(rems(0.0625))
            .bg(theme.border)
            .cursor(CursorStyle::ResizeLeftRight)
            .h_full()
            .flex_none()
            .child(
                deferred(div()
                    .id("operations-list-edge-hit-area")
                    .absolute()
                    .left(rems(-0.0625))
                    .w(rems(0.1875))
                    .h_full()
                    .occlude()
                    .cursor(CursorStyle::ResizeLeftRight)
                    .child(
                        div()
                            .absolute()
                            .left(rems(0.0625))
                            .top_0()
                            .bottom_0()
                            .w(rems(0.0625))
                            .child(resize_highlight("operations-list-edge-highlight", highlighted, theme.accent.opacity(0.6))),
                    )
                    .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                        if this.list_edge_hovered != *hovered {
                            this.list_edge_hovered = *hovered;
                            cx.notify();
                        }
                    }))
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, event, _, cx| {
                        cx.stop_propagation();
                        start(this, event, cx);
                    }))),
            )
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, event, _, cx| start(this, event, cx)))
    }

    /// The edge's drag, as the file chooser's sidebar.
    pub(super) fn resize_list(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        if !event.dragging() {
            self.finish_list_resize(cx);
            cx.notify();
            return;
        }
        // Kept within bounds as shown, so a drag past them does not linger in the setting.
        let shown = event.position.x.as_f32() - self.grab_offset;
        let width = shown.max(rem_pixels(MIN_WIDTH).as_f32()) / self.rem_px.max(1.0);
        if self.list_width != Some(width) {
            self.list_width = Some(width);
            cx.notify();
        }
    }
}
