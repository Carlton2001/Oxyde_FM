use crate::ui::app::{DragTarget, OxydeApp};
use super::ThemeContext;
use super::{InteractiveElement, IntoElement, ParentElement, Styled};
use gpui::*;

impl OxydeApp {
    pub(crate) fn render_layout_node(
        &self,
        node: &crate::ui::workspace::LayoutNode,
        handle: Entity<Self>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match node {
            crate::ui::workspace::LayoutNode::Pane(pane) => {
                let p_id = pane.id.clone();
                div()
                    .flex_1()
                    .min_w_0()
                    .size_full()
                    .flex()
                    .relative()
                    .child(
                        canvas(|bounds, _window, _cx| bounds, {
                            let h = handle.clone();
                            let p_id = p_id.clone();
                            move |_, bounds: Bounds<Pixels>, _, cx| {
                                let size = Size {
                                    width: bounds.size.width.as_f32(),
                                    height: bounds.size.height.as_f32(),
                                };
                                h.update(cx, |this, cx| {
                                    let width_changed = this
                                        .pane_sizes
                                        .get(&p_id)
                                        .is_none_or(|old| (old.width - size.width).abs() > 1.0);
                                    let height_changed = this.pane_sizes.get(&p_id)
                                        .is_some_and(|old| (old.height - size.height).abs() > 1.0);
                                    this.pane_sizes.insert(p_id.clone(), size);
                                    if height_changed {
                                        if let Some(pane) = this.state.find_pane_mut(&p_id) {
                                            pane.active_tab_mut().keep_cursor_visible(size);
                                        }
                                        cx.notify();
                                    }
                                    if width_changed {
                                        if let Some(pane) = this.state.layout.find_pane(&p_id) {
                                            let parts = pane
                                                .active_tab()
                                                .path
                                                .split(['/', '\\'])
                                                .filter(|part| !part.is_empty())
                                                .count();
                                            if parts > 0 {
                                                pane.active_tab()
                                                    .breadcrumb_scroll
                                                    .scroll_to_item(parts * 2 - 2);
                                            }
                                        }
                                        cx.notify();
                                    }
                                });
                            }
                        })
                        .absolute()
                        .size_full(),
                    )
                    .child(self.render_pane(pane, handle, cx))
                    .into_any_element()
            }
            crate::ui::workspace::LayoutNode::Split {
                id,
                axis,
                children,
                weights,
            } => {
                let total_weight: f32 = weights.iter().sum();
                let mut container = div().flex_1().min_w_0().size_full().flex().child(
                    canvas(|bounds, _window, _cx| bounds, {
                        let h = handle.clone();
                        let s_id = id.clone();
                        let axis_copy = axis.clone();
                        move |_, bounds: Bounds<Pixels>, _window, cx| {
                            let size = match axis_copy {
                                crate::ui::workspace::LayoutAxis::Horizontal => {
                                    bounds.size.width.as_f32()
                                }
                                crate::ui::workspace::LayoutAxis::Vertical => {
                                    bounds.size.height.as_f32()
                                }
                            };
                            h.update(cx, |this, _| {
                                this.split_sizes.insert(s_id.clone(), size);
                            });
                        }
                    })
                    .absolute()
                    .size_full(),
                );

                match axis {
                    crate::ui::workspace::LayoutAxis::Horizontal => {
                        container = container.flex_row();
                    }
                    crate::ui::workspace::LayoutAxis::Vertical => {
                        container = container.flex_col();
                    }
                }

                for (i, child) in children.iter().enumerate() {
                    if i > 0 {
                        let separator = match axis {
                            crate::ui::workspace::LayoutAxis::Horizontal => {
                                div()
                                    .group("pane-resize-handle")
                                    .w(rems(0.25))
                                    .mx(rems(-0.125))
                                    .flex_none()
                                    .cursor(CursorStyle::ResizeLeftRight)
                                    .h_full()
                                    .flex()
                                    .justify_center()
                                    .occlude()
                                    .child(
                                        div()
                                            .w(rems(0.0625))
                                            .h_full()
                                            .bg(cx.theme().border)
                                            .group_hover("pane-resize-handle", |s| s.bg(cx.theme().accent.opacity(0.6))),
                                    )
                                    .on_mouse_down(MouseButton::Left, {
                                        let h = handle.clone();
                                        let s_id = id.clone();
                                        let w_copy = weights.clone();
                                        let idx = i - 1;
                                        move |e, _, cx| {
                                            cx.stop_propagation();
                                            h.update(cx, |this, _| {
                                                this.drag_target = Some(DragTarget::PaneResize {
                                                    split_id: s_id.clone(),
                                                    child_idx: idx,
                                                    initial_weights: w_copy.clone(),
                                                });
                                                this.grab_offset = e.position.x.as_f32();
                                            });
                                        }
                                    })
                            }
                            crate::ui::workspace::LayoutAxis::Vertical => div()
                                .group("pane-resize-handle")
                                .h(rems(0.25))
                                .my(rems(-0.125))
                                .occlude()
                                .flex_none()
                                .cursor(CursorStyle::ResizeUpDown)
                                .w_full()
                                .flex()
                                .items_center()
                                .child(
                                    div()
                                        .h(rems(0.0625))
                                        .w_full()
                                        .bg(cx.theme().border)
                                        .group_hover("pane-resize-handle", |s| s.bg(cx.theme().accent.opacity(0.6))),
                                )
                                .on_mouse_down(MouseButton::Left, {
                                    let h = handle.clone();
                                    let s_id = id.clone();
                                    let w_copy = weights.clone();
                                    let idx = i - 1;
                                    move |e, _, cx| {
                                        cx.stop_propagation();
                                        h.update(cx, |this, _| {
                                            this.drag_target = Some(DragTarget::PaneResize {
                                                split_id: s_id.clone(),
                                                child_idx: idx,
                                                initial_weights: w_copy.clone(),
                                            });
                                            this.grab_offset = e.position.y.as_f32();
                                        });
                                    }
                                }),
                        };
                        container = container.child(deferred(separator));
                    }
                    container = container.child(
                        div()
                            .flex_basis(relative(weights[i] / total_weight.max(0.1)))
                            .flex_grow(1.0)
                            .flex_shrink(1.0)
                            .min_w_0()
                            .size_full()
                            .overflow_hidden()
                            .child(self.render_layout_node(child, handle.clone(), cx)),
                    );
                }
                container.into_any_element()
            }
        }
    }

}
