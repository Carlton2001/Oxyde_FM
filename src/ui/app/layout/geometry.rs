use crate::ui::app::OxydeApp;
use crate::ui::workspace::{LayoutAxis, LayoutNode};
use gpui::Size;
use std::collections::HashMap;

// Match the split containers' flex basis and equal flex growth. Separators
// overlap their neighbours, so they do not consume space on either axis.
fn collect_sizes(
    node: &LayoutNode,
    size: Size<f32>,
    panes: &mut HashMap<String, Size<f32>>,
    splits: &mut HashMap<String, f32>,
) {
    match node {
        LayoutNode::Pane(pane) => { panes.insert(pane.id.clone(), size); }
        LayoutNode::Split { id, axis, children, weights } => {
            let total: f32 = weights.iter().sum();
            let denominator = total.max(0.1);
            let growth = (1.0 - total / denominator) / children.len().max(1) as f32;
            splits.insert(id.clone(), match axis {
                LayoutAxis::Horizontal => size.width,
                LayoutAxis::Vertical => size.height,
            });
            for (child, weight) in children.iter().zip(weights) {
                let fraction = weight / denominator + growth;
                let child_size = match axis {
                    LayoutAxis::Horizontal => Size { width: size.width * fraction, ..size },
                    LayoutAxis::Vertical => Size { height: size.height * fraction, ..size },
                };
                collect_sizes(child, child_size, panes, splits);
            }
        }
    }
}

impl OxydeApp {
    /// The viewport and split weights are available before rendering. Derive
    /// pane geometry now instead of painting with the preceding frame's sizes.
    pub(in crate::ui::app) fn sync_layout_sizes(&mut self) {
        let mut panes = HashMap::new();
        let mut splits = HashMap::new();
        collect_sizes(&self.state.layout, Size { width: self.list_width, height: self.list_height }, &mut panes, &mut splits);
        for (id, size) in &panes {
            let previous = self.pane_sizes.get(id);
            if previous.is_some_and(|old| (old.height - size.height).abs() > 1.0) {
                if let Some(pane) = self.state.find_pane_mut(id) {
                    pane.active_tab_mut().keep_cursor_visible(*size);
                }
            }
            if previous.is_none_or(|old| (old.width - size.width).abs() > 1.0) {
                if let Some(pane) = self.state.find_pane(id) {
                    let tab = pane.active_tab();
                    let parts = tab.path.split(std::path::is_separator).filter(|part| !part.is_empty()).count();
                    if parts > 0 { tab.breadcrumb_scroll.scroll_to_item(parts * 2 - 2); }
                }
            }
        }
        self.pane_sizes = panes;
        self.split_sizes = splits;
    }
}
