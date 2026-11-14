//! The grid's card size, changed with Ctrl and the wheel over a grid, and the card
//! under the pointer, whose whole name shows.
use super::OxydeApp;
use crate::ui::date_groups::{content_height, display_rows, row_height, DisplayRow};
use crate::ui::style::{grid_size, set_grid_size, CardLayout, GridSize};
use crate::ui::workspace::ViewMode;
use gpui::*;

impl OxydeApp {
    pub(crate) fn grid_size(&self) -> GridSize {
        GridSize::from_config(&self.config.grid_size)
    }

    /// Ctrl and the wheel over a pane showing the grid. Wheel notches step one size
    /// each; a touchpad steps once its scrolling adds up to a few lines.
    pub(crate) fn grid_size_wheel(&mut self, event: &ScrollWheelEvent, window: &Window, cx: &mut Context<Self>) -> bool {
        let position = event.position;
        let over_grid = self.list_bounds.iter().any(|(pane_id, bounds)| {
            let x = position.x.as_f32();
            let y = position.y.as_f32();
            x >= bounds.origin.x && x < bounds.origin.x + bounds.size.width
                && y >= bounds.origin.y && y < bounds.origin.y + bounds.size.height
                && self.state.find_pane(pane_id).is_some_and(|pane| pane.active_tab().view_mode == ViewMode::Grid)
        });
        if !over_grid {
            return false;
        }
        let steps = match event.delta {
            ScrollDelta::Lines(lines) => {
                self.grid_wheel = 0.0;
                if lines.y > 0.0 { 1 } else if lines.y < 0.0 { -1 } else { 0 }
            }
            ScrollDelta::Pixels(pixels) => {
                let threshold = window.line_height().as_f32() * 3.0;
                if self.grid_wheel.signum() != pixels.y.as_f32().signum() {
                    self.grid_wheel = 0.0;
                }
                self.grid_wheel += pixels.y.as_f32();
                if self.grid_wheel.abs() < threshold {
                    0
                } else {
                    let step = self.grid_wheel.signum() as i32;
                    self.grid_wheel = 0.0;
                    step
                }
            }
        };
        if steps != 0 {
            self.set_grid_size(self.grid_size().step(steps > 0), cx);
        }
        true
    }

    /// Every grid keeps the first entry it showed at the top while its cards change size.
    pub(crate) fn set_grid_size(&mut self, size: GridSize, cx: &mut Context<Self>) {
        let previous = self.grid_size();
        if previous == size {
            return;
        }
        let widths: Vec<(String, f32)> = self.list_bounds.iter().map(|(pane, bounds)| (pane.clone(), bounds.size.width)).collect();
        let mut anchors = Vec::new();
        for (pane_id, width) in &widths {
            let Some(pane) = self.state.find_pane(pane_id) else { continue };
            let tab = pane.active_tab();
            if tab.view_mode != ViewMode::Grid {
                continue;
            }
            anchors.push((pane_id.clone(), *width, first_visible_entry(tab, CardLayout::Grid(previous), *width)));
        }
        self.config.grid_size = size.config_value().to_string();
        set_grid_size(size);
        for (pane_id, width, anchor) in anchors {
            let Some(index) = anchor else { continue };
            let Some(pane) = self.state.find_pane(&pane_id) else { continue };
            let tab = pane.active_tab();
            let layout = CardLayout::Grid(size);
            let rows = display_rows(tab.filtered_files(), tab.grouping(), &tab.collapsed_groups, tab.card_columns(width));
            if let Some(row) = rows.iter().position(|row| matches!(row, DisplayRow::Files(indices) if indices.contains(&index))) {
                let top = content_height(&rows[..row], Some(layout));
                tab.list_scroll.0.borrow().base_handle.set_offset(point(px(0.0), px(-top)));
            }
        }
        self.hovered_card = None;
        self.save_preferences(cx);
        cx.notify();
    }

    pub(crate) fn set_hovered_card(&mut self, pane_id: &str, path: &str, hovered: bool, cx: &mut Context<Self>) {
        let current = self.hovered_card.as_ref().is_some_and(|(pane, card)| pane == pane_id && card == path);
        if hovered && !current {
            self.hovered_card = Some((pane_id.to_string(), path.to_string()));
            cx.notify();
        } else if !hovered && current {
            self.hovered_card = None;
            cx.notify();
        }
    }

    /// Applies the preferences' size before anything lays cards out.
    pub(crate) fn sync_grid_size(&self) {
        let size = self.grid_size();
        if grid_size() != size {
            set_grid_size(size);
        }
    }
}

/// The first entry of the first line of cards still in view, before the size changes.
fn first_visible_entry(tab: &crate::ui::workspace::TabState, layout: CardLayout, width: f32) -> Option<usize> {
    let offset = -tab.list_scroll.0.borrow().base_handle.offset().y.as_f32();
    if offset <= 0.0 {
        return None;
    }
    let columns = crate::ui::style::GridGeometry::for_viewport(layout, width, crate::ui::components::scrollbar::track_width()).columns;
    let rows = display_rows(tab.filtered_files(), tab.grouping(), &tab.collapsed_groups, columns);
    let mut top = 0.0;
    for row in &rows {
        let height = row_height(row, Some(layout));
        if top + height > offset {
            if let DisplayRow::Files(indices) = row {
                return indices.first().copied();
            }
        }
        top += height;
    }
    None
}
