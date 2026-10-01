use crate::ui::scale::{logical, rem_pixels};
use crate::ui::app::OxydeApp;
use crate::ui::components::scrollbar::{render_scrollbar, ScrollbarAxis};
use crate::ui::components::tooltip::Tooltip;
use crate::ui::format::{format_number, format_size};
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::Subscription;
use gpui::*;
use oxyde_lib::systems::file_ops::{FileOpType, FileOperation, OpStatus};

pub(crate) const PANEL_WIDTH: Rems = rems(22.5);
pub(crate) fn panel_width() -> f32 { rem_pixels(PANEL_WIDTH).as_f32() }
const PANEL_MAX_HEIGHT: Rems = rems(26.25);
fn panel_max_height() -> f32 { rem_pixels(PANEL_MAX_HEIGHT).as_f32() }

pub struct OperationsPanelView {
    app: WeakEntity<OxydeApp>,
    _app_subscription: Subscription,
    position: Option<Point<Pixels>>,
    bounds: Option<Bounds<Pixels>>,
    drag_offset: Option<Point<Pixels>>,
    last_drag_update: Option<std::time::Instant>,
}

impl OperationsPanelView {
    pub fn new(app: Entity<OxydeApp>, cx: &mut Context<Self>) -> Self {
        let position = app.read(cx).operations_panel_position;
        let subscription = cx.observe(&app, |_, _, cx| cx.notify());
        Self {
            app: app.downgrade(),
            _app_subscription: subscription,
            position,
            bounds: None,
            drag_offset: None,
            last_drag_update: None,
        }
    }

    pub(crate) fn update_drag(
        &mut self,
        pointer: Point<Pixels>,
        viewport: Size<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some(grab_offset) = self.drag_offset else {
            return;
        };
        let now = std::time::Instant::now();
        if self
            .last_drag_update
            .is_some_and(|last| now.duration_since(last) < std::time::Duration::from_millis(16))
        {
            return;
        }
        let panel_size = self
            .bounds
            .map(|bounds| bounds.size)
            .unwrap_or(size(px(panel_width()), px(panel_max_height())));
        let margin = logical(12.0);
        let max_left = (viewport.width.as_f32() - panel_size.width.as_f32() - margin).max(margin);
        let max_top = (viewport.height.as_f32() - panel_size.height.as_f32() - logical(36.0)).max(margin);
        self.position = Some(point(
            px((pointer.x.as_f32() - grab_offset.x.as_f32()).clamp(margin, max_left)),
            px((pointer.y.as_f32() - grab_offset.y.as_f32()).clamp(margin, max_top)),
        ));
        self.last_drag_update = Some(now);
        cx.notify();
    }

    fn update_scrollbar_drag(&self, pointer: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let (viewport_h, content_h, grab_offset, origin, scroll_handle) = {
            let app = app.read(cx);
            let Some(crate::ui::app::DragTarget::OperationsPanelV {
                viewport_h,
                content_h,
                track_id,
            }) = app.drag_target.as_ref()
            else {
                return;
            };
            let Some(origin) = app.scrollbar_origins.get(track_id) else {
                return;
            };
            (
                *viewport_h,
                *content_h,
                app.grab_offset,
                *origin,
                app.operations_panel_scroll.clone(),
            )
        };
        let mouse_pos_in_track = pointer.y.as_f32() - origin.y;
        let thumb_size = (viewport_h * (viewport_h / content_h.max(1.0)).min(1.0))
            .max(logical(40.0))
            .min(viewport_h);
        let travel_distance = (viewport_h - thumb_size).max(1.0);
        let content_range = (content_h - viewport_h).max(0.0);
        let target_thumb_pos = (mouse_pos_in_track - grab_offset).clamp(0.0, travel_distance);
        let target_offset = (target_thumb_pos / travel_distance) * content_range;
        scroll_handle.set_offset(point(px(0.0), px(-target_offset)));
        cx.notify();
    }

    pub(crate) fn finish_drag(&mut self, cx: &mut Context<Self>) {
        if self.drag_offset.take().is_none() {
            return;
        }
        self.last_drag_update = None;
        if let (Some(app), Some(position)) = (self.app.upgrade(), self.position) {
            app.update(cx, |this, cx| {
                this.persist_operations_panel_position(position, cx);
                cx.notify();
            });
        }
        cx.notify();
    }
}

impl Render for OperationsPanelView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(app_handle) = self.app.upgrade() else {
            return div().into_any_element();
        };
        let panel_handle = cx.entity();
        let position = self.position;
        let bounds = self.bounds;
        let is_dragging = self.drag_offset.is_some();
        let viewport = window.viewport_size();
        app_handle.update(cx, |app, cx| {
            render_operations_panel(
                app,
                app_handle.clone(),
                panel_handle,
                position,
                bounds,
                is_dragging,
                viewport,
                cx,
            )
            .into_any_element()
        })
    }
}

pub fn render_operations_footer(
    app: &OxydeApp,
    handle: Entity<OxydeApp>,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let theme = *cx.theme();
    let operations = app.file_ops.list_operations();
    let active = operations.iter().filter(|op| !is_final(&op.status)).count();
    let count = format_number(active as u64, cx.i18n().language());
    let tooltip = cx.i18n().text(if app.operations_panel_open {
        Message::OpsHidePanel
    } else {
        Message::OpsShowPanel
    });
    div()
        .id("operations-footer")
        .absolute()
        .left(rems(0.5))
        .bottom(rems(0.3125))
        .h(rems(1.75))
        .flex()
        .items_center()
        .gap(rems(0.375))
        .px(rems(0.5))
        .rounded_sm()
        .text_color(theme.text_muted)
        .occlude()
        .tooltip(Tooltip::text(tooltip))
        .hover(|style| style.bg(theme.control_hover).text_color(theme.text))
        .child(
            svg()
                .path("icons/download.svg")
                .text_color(theme.text_muted)
                .size_4(),
        )
        .child(count)
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            handle.update(cx, |this, cx| {
                this.operations_panel_open = !this.operations_panel_open;
                this.operations_panel_opened_manually = this.operations_panel_open;
                cx.notify();
            });
        })
}

pub fn render_operations_panel(
    app: &OxydeApp,
    handle: Entity<OxydeApp>,
    panel_handle: Entity<OperationsPanelView>,
    panel_position: Option<Point<Pixels>>,
    panel_bounds: Option<Bounds<Pixels>>,
    is_dragging: bool,
    viewport_size: Size<Pixels>,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let theme = *cx.theme();
    let operations = app.file_ops.list_operations();
    let mut title_args = FluentArgs::new();
    title_args.set("count", operations.len() as i64);
    let title = cx.i18n().text_with_args(Message::OpsTitle, &title_args);
    let operation_count = format_number(operations.len() as u64, cx.i18n().language());
    let has_finished_operations = operations
        .iter()
        .any(|operation| is_final(&operation.status));
    let operations_manager = app.file_ops.clone();
    let close_handle = handle.clone();
    let panel_size = panel_bounds
        .map(|bounds| bounds.size)
        .unwrap_or(size(px(panel_width()), px(panel_max_height())));
    let saved_position = panel_position.map(|position| {
        let margin = logical(12.0);
        let max_left =
            (viewport_size.width.as_f32() - panel_size.width.as_f32() - margin).max(margin);
        let max_top =
            (viewport_size.height.as_f32() - panel_size.height.as_f32() - logical(36.0)).max(margin);
        point(
            px(position.x.as_f32().clamp(margin, max_left)),
            px(position.y.as_f32().clamp(margin, max_top)),
        )
    });
    let mut panel = div()
        .id("operations-panel")
        .absolute()
        .w(px(panel_width()))
        .max_h(px(panel_max_height()))
        .flex()
        .flex_col()
        .text_size(rems(0.75))
        .overflow_hidden()
        .rounded_md()
        .border(rems(0.0625))
        .border_color(theme.menu_border)
        .bg(theme.menu_surface)
        .shadow(crate::ui::style::rem_shadow(true))
        .occlude()
        .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
        .on_mouse_move({
            let panel_handle = panel_handle.clone();
            move |event, window, cx| {
                if !event.dragging() {
                    panel_handle.update(cx, |panel, cx| panel.finish_drag(cx));
                    if let Some(app) = panel_handle.read(cx).app.upgrade() {
                        app.update(cx, |app, cx| app.finish_pointer_drag(cx));
                    }
                    return;
                }
                let pointer = event.position;
                let viewport = window.viewport_size();
                panel_handle.update(cx, |this, cx| {
                    this.update_drag(pointer, viewport, cx);
                    this.update_scrollbar_drag(pointer, cx);
                });
            }
        })
        .capture_any_mouse_up({
            let app_handle = handle.clone();
            let panel_handle = panel_handle.clone();
            move |event, _, cx| {
                if event.button == MouseButton::Left {
                    panel_handle.update(cx, |this, cx| {
                        this.finish_drag(cx);
                    });
                    app_handle.update(cx, |app, cx| {
                        if matches!(
                            app.drag_target,
                            Some(crate::ui::app::DragTarget::OperationsPanelV { .. })
                        ) {
                            app.drag_target = None;
                            cx.notify();
                        }
                    });
                }
            }
        })
        .on_mouse_up_out(MouseButton::Left, {
            let app_handle = handle.clone();
            let panel_handle = panel_handle.clone();
            move |_, _, cx| {
                panel_handle.update(cx, |this, cx| {
                    this.finish_drag(cx);
                });
                app_handle.update(cx, |app, cx| {
                    if matches!(
                        app.drag_target,
                        Some(crate::ui::app::DragTarget::OperationsPanelV { .. })
                    ) {
                        app.drag_target = None;
                        cx.notify();
                    }
                });
            }
        })
        .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
        .child({
            let panel_handle = panel_handle.clone();
            canvas(
                |bounds, _, _| bounds,
                move |bounds, _, _, cx| {
                    panel_handle.update(cx, |this, cx| {
                        if this.bounds != Some(bounds) {
                            this.bounds = Some(bounds);
                            cx.notify();
                        }
                    });
                },
            )
            .absolute()
            .size_full()
        })
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .h(rems(2.125))
                .px(rems(0.625))
                .border_b(rems(0.0625))
                .border_color(theme.border)
                .text_size(rems(0.75))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(rems(0.375))
                        .cursor(if is_dragging {
                            CursorStyle::ClosedHand
                        } else {
                            CursorStyle::OpenHand
                        })
                        .child(
                            svg()
                                .path("icons/download.svg")
                                .text_color(theme.text)
                                .size_4(),
                        )
                        .child(operation_count)
                        .child(title),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(rems(0.25))
                        .when(has_finished_operations, |actions| {
                            let manager = operations_manager.clone();
                            let handle = panel_handle.clone();
                            actions.child(
                                div()
                                    .id("operations-panel-clear")
                                    .w(rems(1.5))
                                    .h(rems(1.5))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_sm()
                                    .hover(|style| style.bg(theme.control_hover))
                                    .tooltip(Tooltip::text(cx.i18n().text(Message::OpsDismissAll)))
                                    .child(
                                        svg()
                                            .path("icons/broom.svg")
                                            .text_color(theme.icon_muted)
                                            .size_4(),
                                    )
                                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                        cx.stop_propagation();
                                        manager.dismiss_finished_operations();
                                        handle.update(cx, |_, cx| cx.notify());
                                    }),
                            )
                        })
                        .child(
                            div()
                                .id("operations-panel-close")
                                .w(rems(1.5))
                                .h(rems(1.5))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_sm()
                                .hover(|style| style.bg(theme.control_hover))
                                .tooltip(Tooltip::text(cx.i18n().text(Message::OpsHidePanel)))
                                .child(
                                    svg()
                                        .path("icons/chevron-down.svg")
                                        .text_color(theme.icon_muted)
                                        .size_4(),
                                )
                                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                    cx.stop_propagation();
                                    close_handle.update(cx, |this, cx| {
                                        this.operations_panel_open = false;
                                        this.operations_panel_opened_manually = false;
                                        cx.notify();
                                    });
                                }),
                        ),
                )
                .cursor(if is_dragging {
                    CursorStyle::ClosedHand
                } else {
                    CursorStyle::OpenHand
                })
                .on_mouse_down(MouseButton::Left, {
                    let drag_handle = panel_handle.clone();
                    move |event, _, cx| {
                        cx.stop_propagation();
                        let pointer = event.position;
                        drag_handle.update(cx, |this, cx| {
                            if let Some(bounds) = this.bounds {
                                this.drag_offset = Some(point(
                                    pointer.x - bounds.left(),
                                    pointer.y - bounds.top(),
                                ));
                                this.last_drag_update = None;
                                cx.notify();
                            }
                        });
                    }
                }),
        );
    if let Some(position) = saved_position {
        panel = panel.left(position.x).top(position.y);
    } else {
        panel = panel.right(rems(0.75)).bottom(rems(2.25));
    }
    if operations.is_empty() {
        panel = panel.child(
            div()
                .p(rems(0.75))
                .text_color(theme.text_muted)
                .child(cx.i18n().text(Message::OpsEmpty)),
        );
    } else {
        let cards = operations
            .into_iter()
            .rev()
            .enumerate()
            .map(|(index, operation)| {
                render_operation_card(
                    app,
                    operation,
                    index % 2 == 1,
                    app.handle.clone().expect("handle set"),
                    cx,
                )
            })
            .collect::<Vec<_>>();
        let scroll_handle = app.operations_panel_scroll.clone();
        let viewport_h = scroll_handle.bounds().size.height.as_f32();
        let content_h = viewport_h + scroll_handle.max_offset().y.as_f32();
        let scroll_offset = -scroll_handle.offset().y.as_f32();
        let scroll_viewport = div()
            .id("operations-list")
            .flex_1()
            .min_h_0()
            .pr(px(crate::ui::components::scrollbar::zed_track_width()))
            .overflow_y_scroll()
            .track_scroll(&scroll_handle)
            .children(cards);
        panel = panel.child(scroll_viewport).child(
            div()
                .absolute()
                .top(rems(2.125))
                .right_0()
                .bottom_0()
                .w(px(crate::ui::components::scrollbar::zed_track_width()))
                .child(render_scrollbar(
                    theme,
                    handle,
                    ScrollbarAxis::Vertical,
                    viewport_h,
                    content_h,
                    scroll_offset,
                    matches!(
                        app.drag_target,
                        Some(crate::ui::app::DragTarget::OperationsPanelV { .. })
                    ),
                    "operations-panel".to_string(),
                    None,
                    |this, track_id, viewport_h, content_h, _| {
                        this.drag_target = Some(crate::ui::app::DragTarget::OperationsPanelV {
                            viewport_h,
                            content_h,
                            track_id,
                        });
                    },
                )),
        );
    }
    panel
}

fn is_final(status: &OpStatus) -> bool {
    matches!(
        status,
        OpStatus::Completed | OpStatus::Cancelled | OpStatus::Error(_)
    )
}

fn format_elapsed_duration(duration_ms: u64, language: crate::ui::i18n::Language) -> String {
    let total_seconds = duration_ms / 1_000;
    if total_seconds < 60 {
        let tenths = duration_ms.saturating_add(50) / 100;
        let decimal = match language {
            crate::ui::i18n::Language::French => ',',
            crate::ui::i18n::Language::English => '.',
        };
        return format!(
            "{}{decimal}{} s",
            format_number(tenths / 10, language),
            tenths % 10
        );
    }
    if total_seconds < 3_600 {
        return format!(
            "{} min {} s",
            format_number(total_seconds / 60, language),
            format_number(total_seconds % 60, language)
        );
    }
    format!(
        "{} h {} min",
        format_number(total_seconds / 3_600, language),
        format_number((total_seconds % 3_600) / 60, language)
    )
}

fn speed_graph(samples: Vec<u64>, accent: Hsla) -> impl IntoElement {
    canvas(
        |bounds, _, _| bounds,
        move |_, bounds: Bounds<Pixels>, window, _| {
            let width = bounds.size.width.as_f32();
            let height = bounds.size.height.as_f32();
            if samples.is_empty() || width <= logical(2.0) || height <= logical(4.0) {
                return;
            }
            let left = bounds.origin.x.as_f32() + logical(1.0);
            let right = left + width - logical(2.0);
            let top = bounds.origin.y.as_f32() + logical(2.0);
            let bottom = top + height - logical(4.0);
            let max_speed = samples.iter().copied().max().unwrap_or(0).max(1) as f64;
            let points: Vec<_> = if samples.len() == 1 {
                let y = bottom - (samples[0] as f64 / max_speed) as f32 * (bottom - top);
                vec![point(px(left), px(y)), point(px(right), px(y))]
            } else {
                samples
                    .iter()
                    .enumerate()
                    .map(|(index, speed)| {
                        let x = left + (right - left) * index as f32 / (samples.len() - 1) as f32;
                        let y = bottom - (*speed as f64 / max_speed) as f32 * (bottom - top);
                        point(px(x), px(y))
                    })
                    .collect()
            };
            let mut area = PathBuilder::fill();
            area.move_to(point(px(left), px(bottom)));
            for point in &points {
                area.line_to(*point);
            }
            area.line_to(point(px(right), px(bottom)));
            area.close();
            if let Ok(path) = area.build() {
                window.paint_path(path, accent.opacity(0.08));
            }
            let mut line = PathBuilder::stroke(rem_pixels(rems(0.09375)));
            line.move_to(points[0]);
            for point in points.iter().skip(1) {
                line.line_to(*point);
            }
            if let Ok(path) = line.build() {
                window.paint_path(path, accent.opacity(0.28));
            }
        },
    )
    .absolute()
    .inset_0()
}

fn render_operation_card(
    app: &OxydeApp,
    operation: FileOperation,
    alternate_background: bool,
    handle: Entity<OxydeApp>,
    cx: &mut Context<OxydeApp>,
) -> AnyElement {
    let theme = *cx.theme();
    let i18n = cx.i18n();
    let language = i18n.language();
    let (kind, icon) = match operation.op_type {
        FileOpType::Copy => (Message::OpCopy, "icons/copy.svg"),
        FileOpType::Move => (Message::OpMove, "icons/arrow-right.svg"),
        FileOpType::Delete => (Message::OpDelete, "icons/delete.svg"),
        FileOpType::Trash => (Message::OpDelete, "icons/trash.svg"),
        FileOpType::Compress => (Message::OpCompress, "icons/archive.svg"),
        FileOpType::Extract => (Message::OpExtract, "icons/archive-restore.svg"),
    };
    let state = match operation.status {
        OpStatus::Queued => Message::OpsQueued,
        OpStatus::Calculating => Message::OpsCalculating,
        OpStatus::Running => Message::OpsRunning,
        OpStatus::Paused => Message::OpsPaused,
        OpStatus::Cancelled => Message::OpsCancelled,
        OpStatus::Completed => Message::OpsCompleted,
        OpStatus::Error(_) => Message::OpsError,
    };
    let final_state = is_final(&operation.status);
    let compact_summary = matches!(operation.status, OpStatus::Completed | OpStatus::Cancelled);
    let completed = matches!(operation.status, OpStatus::Completed);
    let waiting = matches!(operation.status, OpStatus::Queued | OpStatus::Paused);
    let unmanaged_extraction = matches!(operation.op_type, FileOpType::Extract)
        && operation.sources.first().is_some_and(|path| {
            !path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"))
        })
        && matches!(operation.status, OpStatus::Running | OpStatus::Paused);
    let source = operation
        .current_file
        .as_ref()
        .map(std::path::PathBuf::from)
        .or_else(|| operation.sources.first().cloned())
        .unwrap_or_default();
    let mut from_args = FluentArgs::new();
    from_args.set("path", source.to_string_lossy().into_owned());
    let from = i18n.text_with_args(Message::OpsFrom, &from_args);
    let to = operation.destination.as_ref().map(|destination| {
        let mut args = FluentArgs::new();
        args.set("path", destination.to_string_lossy().into_owned());
        i18n.text_with_args(Message::OpsTo, &args)
    });
    let mut items_args = FluentArgs::new();
    items_args.set(
        "done",
        format_number(operation.processed_files as u64, language),
    );
    items_args.set(
        "total",
        format_number(operation.total_files as u64, language),
    );
    let items = i18n.text_with_args(Message::OpsItems, &items_args);
    let ratio = if operation.total_bytes > 0 {
        Some(operation.processed_bytes as f64 / operation.total_bytes as f64)
    } else if operation.total_files > 0 {
        Some(operation.processed_files as f64 / operation.total_files as f64)
    } else {
        None
    };
    let percentage = ratio.map(|value| (value * 100.0).clamp(0.0, 100.0).round() as u32);
    let progress_text = if operation.total_bytes > 0 {
        Some(format!(
            "{} / {}",
            format_size(operation.processed_bytes, false, true, language),
            format_size(operation.total_bytes, false, true, language)
        ))
    } else {
        None
    };
    let displayed_speed = if final_state {
        operation.duration_ms.and_then(|duration_ms| {
            (duration_ms > 0 && operation.processed_bytes > 0)
                .then(|| operation.processed_bytes.saturating_mul(1_000) / duration_ms)
        })
    } else {
        (operation.bytes_per_second > 0).then_some(operation.bytes_per_second)
    };
    let speed = displayed_speed.map(|bytes_per_second| {
        let value = format!("{}/s", format_size(bytes_per_second, false, true, language));
        if final_state {
            let mut args = FluentArgs::new();
            args.set("speed", value);
            i18n.text_with_args(Message::OpsAverage, &args).to_string()
        } else {
            value
        }
    });
    let remaining = if operation.bytes_per_second > 0
        && operation.total_bytes > operation.processed_bytes
        && !final_state
    {
        let seconds =
            (operation.total_bytes - operation.processed_bytes) / operation.bytes_per_second;
        let mut args = FluentArgs::new();
        args.set("seconds", format_number(seconds, language));
        Some(i18n.text_with_args(Message::OpsRemaining, &args))
    } else {
        None
    };
    let id = operation.id.clone();
    let pause_id = id.clone();
    let turbo_id = id.clone();
    let cancel_id = id.clone();
    let paused = matches!(operation.status, OpStatus::Paused);
    let can_start = paused || matches!(operation.status, OpStatus::Queued);
    let turbo = operation.turbo;
    let manager = app.file_ops.clone();
    let pause_manager = manager.clone();
    let turbo_manager = manager.clone();
    let cancel_manager = manager.clone();
    let mut status_row = div()
        .flex()
        .flex_none()
        .items_center()
        .gap(rems(0.25))
        .text_color(theme.text_muted);
    if unmanaged_extraction || waiting {
        status_row = status_row.child(
            div()
                .when(waiting, |label| label.text_color(theme.accent))
                .child(i18n.text(state)),
        );
    }
    if final_state {
        let dismiss_id = id.clone();
        let dismiss_handle = handle.clone();
        let dismiss_manager = manager.clone();
        status_row = status_row.child(
            div()
                .id(format!("dismiss-{dismiss_id}"))
                .px(rems(0.3125))
                .h(rems(1.25))
                .flex()
                .items_center()
                .justify_center()
                .gap(rems(0.25))
                .rounded_sm()
                .hover(|style| style.bg(theme.menu_hover).text_color(theme.text))
                .tooltip(Tooltip::text(i18n.text(Message::OpsDismiss)))
                .child(
                    svg()
                        .path("icons/x.svg")
                        .text_color(theme.icon_muted)
                        .size_3(),
                )
                .child(i18n.text(state))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    dismiss_manager.dismiss_operation(&dismiss_id);
                    dismiss_handle.update(cx, |_, cx| cx.notify());
                }),
        );
    } else if !unmanaged_extraction {
        let pause_handle = handle.clone();
        let turbo_handle = handle.clone();
        let pause_icon = if can_start {
            "icons/play.svg"
        } else {
            "icons/pause.svg"
        };
        let turbo_icon = if turbo {
            "icons/zap.svg"
        } else {
            "icons/zap-off.svg"
        };
        status_row = status_row
            .child(
                div()
                    .id(format!("pause-{pause_id}"))
                    .w(rems(1.375))
                    .h(rems(1.375))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_sm()
                    .hover(|style| style.bg(theme.control_hover))
                    .tooltip(Tooltip::text(i18n.text(if can_start {
                        Message::OpsResume
                    } else {
                        Message::OpsPause
                    })))
                    .child(svg().path(pause_icon).text_color(theme.text).size_4())
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        pause_manager.set_paused(&pause_id, !can_start);
                        pause_handle.update(cx, |_, cx| cx.notify());
                    }),
            )
            .child(
                div()
                    .id(format!("turbo-{turbo_id}"))
                    .w(rems(1.375))
                    .h(rems(1.375))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_sm()
                    .hover(|style| style.bg(theme.control_hover))
                    .tooltip(Tooltip::text(i18n.text(if turbo {
                        Message::OpsFast
                    } else {
                        Message::OpsNormal
                    })))
                    .child(
                        svg()
                            .path(turbo_icon)
                            .text_color(if turbo {
                                theme.accent
                            } else {
                                theme.text_muted
                            })
                            .size_4(),
                    )
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        turbo_manager.set_turbo(&turbo_id, !turbo);
                        turbo_handle.update(cx, |_, cx| cx.notify());
                    }),
            )
            .child(
                div()
                    .id(format!("cancel-{cancel_id}"))
                    .w(rems(1.375))
                    .h(rems(1.375))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_sm()
                    .hover(|style| style.bg(theme.control_hover))
                    .tooltip(Tooltip::text(i18n.text(Message::OpsStop)))
                    .child(
                        svg()
                            .path("icons/x.svg")
                            .text_color(theme.text_muted)
                            .size_4(),
                    )
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        cancel_manager.cancel_operation(&cancel_id);
                        handle.update(cx, |_, cx| cx.notify());
                    }),
            );
    }

    let card = div()
        .id(format!("operation-{id}"))
        .relative()
        .p(rems(0.75))
        .border_b(rems(0.0625))
        .border_color(theme.border)
        .bg(if alternate_background {
            theme.background
        } else {
            theme.menu_surface
        })
        .flex()
        .flex_col()
        .gap(rems(0.3125))
        .when(waiting, |card| {
            card.child(
                div()
                    .absolute()
                    .inset_0()
                    .border_l(rems(0.125))
                    .border_color(theme.accent)
                    .bg(theme.accent.opacity(0.08)),
            )
        })
        .child(
            div()
                .flex()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .flex()
                        .min_w_0()
                        .items_center()
                        .gap(rems(0.375))
                        .child(svg().path(icon).text_color(theme.text).size_4().flex_none())
                        .child(i18n.text(kind))
                        .child(
                            div()
                                .text_color(theme.text_muted)
                                .overflow_hidden()
                                .child(items),
                        ),
                )
                .child(status_row),
        );
    let mut body = div()
        .relative()
        .font_weight(FontWeight::NORMAL)
        .text_color(theme.text_muted)
        .flex()
        .flex_col()
        .gap(rems(0.3125))
        .overflow_hidden();
    if !final_state && !operation.speed_history.is_empty() {
        body = body.child(speed_graph(operation.speed_history.clone(), theme.accent));
    }
    body = body.child(
        div()
            .text_color(theme.text_muted)
            .overflow_hidden()
            .child(from),
    );
    if let Some(to) = to {
        body = body.child(
            div()
                .text_color(theme.text_muted)
                .overflow_hidden()
                .child(to),
        );
    }
    if progress_text.is_some() || (percentage.is_some() && !compact_summary) {
        body = body.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(rems(0.5))
                .child(
                    div()
                        .min_w_0()
                        .overflow_hidden()
                        .child(progress_text.unwrap_or_default()),
                )
                .when(completed, |row| {
                    row.when_some(speed.clone(), |row, speed| {
                        row.child(div().flex_none().child(speed))
                    })
                })
                .when(!compact_summary, |row| {
                    row.child(
                        div()
                            .flex_none()
                            .text_color(theme.text)
                            .child(
                                percentage
                                    .map(|value| format!("{value}%"))
                                    .unwrap_or_default(),
                            ),
                    )
                }),
        );
    }
    let speed = speed.filter(|_| !completed);
    let temporal = if final_state {
        operation.duration_ms.map(|duration_ms| {
            let mut args = FluentArgs::new();
            args.set("duration", format_elapsed_duration(duration_ms, language));
            i18n.text_with_args(Message::OpsDuration, &args)
        })
    } else {
        remaining
    };
    if compact_summary {
        body = body.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(rems(0.5))
                .child(
                    div()
                        .flex_none()
                        .text_color(theme.text)
                        .child(
                            percentage
                                .unwrap_or(if matches!(operation.status, OpStatus::Completed) {
                                    100
                                } else {
                                    0
                                })
                                .to_string()
                                + "%",
                        ),
                )
                .child(
                    div()
                        .text_color(theme.text_muted)
                        .child(temporal.clone().unwrap_or_default()),
                ),
        );
    }
    if !final_state || speed.is_some() || (temporal.is_some() && !compact_summary) {
        body = body.child(
            div()
                .flex()
                .justify_between()
                .child(speed.unwrap_or_else(|| {
                    if final_state {
                        String::new()
                    } else {
                        i18n.text(Message::OpsSpeedWaiting).to_string()
                    }
                }))
                .when(!compact_summary, |row| {
                    row.child(
                        div()
                            .text_color(theme.text_muted)
                            .child(temporal.unwrap_or_default()),
                    )
                }),
        );
    }
    if let OpStatus::Error(error) = &operation.status {
        body = body.child(div().text_color(theme.error).child(error.clone()));
    }
    if let Some(percentage) = percentage.filter(|_| !compact_summary) {
        body = body.child(
            div()
                .w_full()
                .h(rems(0.25))
                .overflow_hidden()
                .bg(theme.control_hover)
                .child(
                    div()
                        .w(relative(percentage as f32 / 100.0))
                        .h_full()
                        .bg(theme.accent.opacity(0.48)),
                ),
        );
    }
    card.child(body).into_any_element()
}
