use crate::ui::app::{DragTarget, OxydeApp};
use crate::ui::elements::resize_highlight::resize_highlight;
use super::ThemeContext;
use super::{InteractiveElement, IntoElement, ParentElement, Styled};
use gpui::*;

/// Whether a pane is the last of every split holding it, down to the root.
fn ends_layout(node: &crate::ui::workspace::LayoutNode, pane_id: &str) -> Option<bool> {
    match node {
        crate::ui::workspace::LayoutNode::Pane(pane) => (pane.id == pane_id).then_some(true),
        crate::ui::workspace::LayoutNode::Split { children, .. } => {
            let last = children.len().saturating_sub(1);
            children.iter().enumerate().find_map(|(index, child)| {
                ends_layout(child, pane_id).map(|ends| ends && index == last)
            })
        }
    }
}

impl OxydeApp {
    /// Radius of a pane's bottom right corner: the sidebar keeps it away from the left one.
    pub(crate) fn pane_corner(&self, pane_id: &str) -> Pixels {
        if ends_layout(&self.state.layout, pane_id).unwrap_or(false) {
            self.window_corners.bottom_right
        } else {
            px(0.0)
        }
    }

    pub(crate) fn render_layout_node(
        &self,
        node: &crate::ui::workspace::LayoutNode,
        handle: Entity<Self>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match node {
            crate::ui::workspace::LayoutNode::Pane(pane) => {
                div()
                    .flex_1()
                    .min_w_0()
                    .size_full()
                    .flex()
                    .relative()
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
                let mut container = div().flex_1().min_w_0().size_full().flex();

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
                        let key = format!("pane-resize-{id}-{}", i - 1);
                        let highlighted = self.hovered_separator.as_deref() == Some(key.as_str())
                            || matches!(&self.drag_target, Some(DragTarget::PaneResize { split_id, child_idx, .. }) if split_id == id && *child_idx == i - 1);
                        let separator = match axis {
                            crate::ui::workspace::LayoutAxis::Horizontal => {
                                div()
                                    .id(SharedString::from(key.clone()))
                                    .on_hover({ let h = handle.clone(); let key = key.clone(); move |hovered, _, cx| h.update(cx, |this, cx| this.hover_separator(&key, *hovered, cx)) })
                                    .w(rems(0.25))
                                    .mx(rems(-0.125))
                                    .flex_none()
                                    .cursor(CursorStyle::ResizeLeftRight)
                                    .h_full()
                                    .flex()
                                    // Keep the line on the pane before the handle: the next pane paints
                                    // later and its opaque bars would otherwise cut it.
                                    .justify_end()
                                    .pr(rems(0.125))
                                    .occlude()
                                    .child(
                                        div()
                                            .relative()
                                            .w(rems(0.0625))
                                            .h_full()
                                            .bg(cx.theme().border)
                                            .child(resize_highlight(format!("{key}-highlight"), highlighted, cx.theme().accent.opacity(0.6))),
                                    )
                                    .on_mouse_down(MouseButton::Left, {
                                        let h = handle.clone();
                                        let s_id = id.clone();
                                        let w_copy = weights.clone();
                                        let idx = i - 1;
                                        move |e, _, cx| {
                                            cx.stop_propagation();
                                            h.update(cx, |this, cx| {
                                                let weights = if e.click_count == 2 { this.equalize_split(&s_id, idx, cx) } else { None };
                                                this.drag_target = Some(DragTarget::PaneResize {
                                                    split_id: s_id.clone(),
                                                    child_idx: idx,
                                                    initial_weights: weights.unwrap_or_else(|| w_copy.clone()),
                                                });
                                                this.grab_offset = e.position.x.as_f32();
                                            });
                                        }
                                    })
                            }
                            crate::ui::workspace::LayoutAxis::Vertical => div()
                                .id(SharedString::from(key.clone()))
                                    .on_hover({ let h = handle.clone(); let key = key.clone(); move |hovered, _, cx| h.update(cx, |this, cx| this.hover_separator(&key, *hovered, cx)) })
                                .h(rems(0.25))
                                .my(rems(-0.125))
                                .occlude()
                                .flex_none()
                                .cursor(CursorStyle::ResizeUpDown)
                                .w_full()
                                .flex()
                                .items_end()
                                .pb(rems(0.125))
                                .child(
                                    div()
                                        .relative()
                                        .h(rems(0.0625))
                                        .w_full()
                                        .bg(cx.theme().border)
                                        .child(resize_highlight(format!("{key}-highlight"), highlighted, cx.theme().accent.opacity(0.6))),
                                )
                                .on_mouse_down(MouseButton::Left, {
                                    let h = handle.clone();
                                    let s_id = id.clone();
                                    let w_copy = weights.clone();
                                    let idx = i - 1;
                                    move |e, _, cx| {
                                        cx.stop_propagation();
                                        h.update(cx, |this, cx| {
                                            let weights = if e.click_count == 2 { this.equalize_split(&s_id, idx, cx) } else { None };
                                            this.drag_target = Some(DragTarget::PaneResize {
                                                split_id: s_id.clone(),
                                                child_idx: idx,
                                                initial_weights: weights.unwrap_or_else(|| w_copy.clone()),
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

    /// Records the separator under the pointer, or forgets it when the pointer leaves.
    pub(crate) fn hover_separator(&mut self, key: &str, hovered: bool, cx: &mut Context<Self>) {
        if hovered {
            self.hovered_separator = Some(SharedString::from(key.to_string()));
        } else if self.hovered_separator.as_deref() == Some(key) {
            self.hovered_separator = None;
        } else {
            return;
        }
        cx.notify();
    }

    /// Double click on a split's handle: share the two neighbouring panes' space equally.
    fn equalize_split(&mut self, split_id: &str, index: usize, cx: &mut Context<Self>) -> Option<Vec<f32>> {
        let (_, weights) = self.state.layout.find_split_mut(split_id)?;
        if index + 1 >= weights.len() { return None; }
        let half = (weights[index] + weights[index + 1]) / 2.0;
        weights[index] = half;
        weights[index + 1] = half;
        let weights = weights.clone();
        cx.notify();
        Some(weights)
    }
}
