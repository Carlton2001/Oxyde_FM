use crate::ui::app::{DragTarget, OxydeApp};
use crate::ui::elements::resize_separator::resize_separator;
use super::{IntoElement, ParentElement, Styled};
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
                        let idx = i - 1;
                        let dragging = matches!(&self.drag_target, Some(DragTarget::PaneResize { split_id, child_idx, .. }) if split_id == id && *child_idx == idx);
                        let horizontal = matches!(axis, crate::ui::workspace::LayoutAxis::Horizontal);
                        // The line covers the last pixel of the pane before it.
                        container = container.child(
                            resize_separator(format!("pane-resize-{id}-{idx}"), if horizontal { Axis::Horizontal } else { Axis::Vertical })
                                .overlap()
                                .dragging(dragging)
                                .on_press({
                                    let h = handle.clone();
                                    let s_id = id.clone();
                                    let w_copy = weights.clone();
                                    move |e, _, cx| {
                                        h.update(cx, |this, cx| {
                                            let weights = if e.click_count == 2 { this.equalize_split(&s_id, idx, cx) } else { None };
                                            this.drag_target = Some(DragTarget::PaneResize {
                                                split_id: s_id.clone(),
                                                child_idx: idx,
                                                initial_weights: weights.unwrap_or_else(|| w_copy.clone()),
                                            });
                                            this.grab_offset = if horizontal { e.position.x.as_f32() } else { e.position.y.as_f32() };
                                        });
                                    }
                                }),
                        );
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

    /// Double click on a split's handle: share the two neighbouring panes' space equally.
    fn equalize_split(&mut self, split_id: &str, index: usize, cx: &mut Context<Self>) -> Option<Vec<f32>> {
        let (_, weights) = self.state.layout.find_split_mut(split_id)?;
        if !equalize_pair(weights, index) { return None; }
        let weights = weights.clone();
        cx.notify();
        Some(weights)
    }
}

/// Shares the space of the panes on each side of separator `index` equally,
/// leaving the others as they are; false when there is no such separator.
fn equalize_pair(weights: &mut [f32], index: usize) -> bool {
    if index + 1 >= weights.len() { return false; }
    let half = (weights[index] + weights[index + 1]) / 2.0;
    weights[index] = half;
    weights[index + 1] = half;
    true
}

#[cfg(test)]
#[path = "tests/tree.rs"]
mod tests;
