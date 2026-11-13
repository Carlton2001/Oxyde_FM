//! The selection rectangle, when several entries may be chosen, as the main window
//! draws and applies its own (`selection.rs`, `handle_selection_drag`): pressed beside
//! the entries, it selects those it covers, with Shift added to the selection before
//! it, with Ctrl toggled against it, and scrolls the list near its edges.
use super::FileChooserWindow;
use crate::ui::app::selection::{autoscroll_depth, rectangle_selection, AUTOSCROLL_RATE};
use crate::ui::theme::OxydeTheme;
use gpui::prelude::*;
use gpui::*;
use std::collections::HashSet;
use std::time::Instant;

pub(super) struct Band {
    /// Where it was pressed, kept on the content as the list scrolls.
    start: Point<Pixels>,
    current: Point<Pixels>,
    modifiers: Modifiers,
    /// The selection before the press, which the modifiers combine with.
    initial: HashSet<usize>,
    /// The list's offset when last applied, and when it last scrolled by itself.
    scroll_y: f32,
    /// Whether the pointer moved far enough to draw it: a plain press selects nothing.
    moved: bool,
    /// Whether the entries' tooltip went when it started to be drawn.
    tooltip_gone: bool,
    autoscroll_at: Option<Instant>,
    _ticker: Option<Task<()>>,
}

impl FileChooserWindow {
    /// The list and grid scroll their own handles.
    fn entries_scroll(&self) -> ScrollHandle {
        if self.options.grid { self.grid_scroll.clone() } else { self.list.0.borrow().base_handle.clone() }
    }

    /// A press beside the entries: nothing selected unless Ctrl or Shift keeps it, and
    /// where several may be chosen, a rectangle starts there.
    pub(super) fn press_background(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        let keep = self.multiple && (event.modifiers.control || event.modifiers.shift);
        if !keep && self.selection.clear() {
            cx.notify();
        }
        if self.multiple {
            self.band = Some(Band {
                start: event.position,
                current: event.position,
                modifiers: event.modifiers,
                initial: self.selection.positions().collect(),
                scroll_y: self.entries_scroll().offset().y.as_f32(),
                moved: false,
                tooltip_gone: false,
                autoscroll_at: None,
                _ticker: None,
            });
            cx.notify();
        }
    }

    pub(super) fn move_band(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) -> bool {
        let Some(band) = &mut self.band else { return false };
        if !event.dragging() {
            self.band = None;
        } else {
            band.current = event.position;
            // The main window's threshold between a press and a drag.
            band.moved |= (band.current - band.start).magnitude() > crate::ui::scale::logical(4.0) as f64;
        }
        cx.notify();
        true
    }

    pub(super) fn end_band(&mut self, cx: &mut Context<Self>) {
        if self.band.take().is_some() {
            cx.notify();
        }
    }

    /// Scrolls near the edges and selects what the rectangle covers, at every frame
    /// while it is held.
    pub(super) fn apply_band(&mut self, cx: &mut Context<Self>) {
        let bounds = self.entries_bounds.get();
        let grid = self.options.grid;
        let header = if grid { 0.0 } else { crate::ui::style::column_header_height() };
        let row_height = if grid { crate::ui::style::grid_card_row_height() } else { crate::ui::style::row_height() };
        let columns = if grid { self.grid_geometry().columns.max(1) } else { 1 };
        let rows = self.shown.len().div_ceil(columns);
        let scroll = self.entries_scroll();
        let geometry = self.grid_geometry();
        let Some(band) = self.band.as_mut().filter(|band| band.moved) else { return };

        // Autoscroll, as GTK list views do: faster the deeper the pointer is in the edge band.
        let (top, bottom) = (bounds.origin.y.as_f32() + header, bounds.bottom().as_f32());
        let depth = autoscroll_depth(band.current.y.as_f32(), top, bottom);
        let mut scroll_y = scroll.offset().y.as_f32();
        if depth == 0.0 {
            band.autoscroll_at = None;
            band._ticker = None;
        } else {
            let now = Instant::now();
            let elapsed = band.autoscroll_at.replace(now).map_or(1.0 / 60.0, |last| now.duration_since(last).as_secs_f32().min(0.1));
            let content = rows as f32 * row_height + crate::ui::style::row_height();
            let maximum = (content - (bottom - top).max(1.0)).max(0.0);
            scroll_y = (scroll_y - depth * AUTOSCROLL_RATE * elapsed).clamp(-maximum, 0.0);
            scroll.set_offset(point(px(0.0), px(scroll_y)));
            // A still pointer sends no events: redraws continue until it leaves the band.
            if band._ticker.is_none() {
                band._ticker = Some(cx.spawn(async move |this, cx| loop {
                    cx.background_executor().timer(std::time::Duration::from_millis(16)).await;
                    if this.update(cx, |_, cx| cx.notify()).is_err() { break; }
                }));
            }
        }
        // Wheel and autoscroll alike move the press with the content.
        band.start.y += px(scroll_y - std::mem::replace(&mut band.scroll_y, scroll_y));

        let (start, current) = (band.start.map(|value| value.as_f32()), band.current.map(|value| value.as_f32()));
        let (left, right) = (start.x.min(current.x), start.x.max(current.x));
        let (upper, lower) = (start.y.min(current.y), start.y.max(current.y));
        // The grid's cards start inside its 0.5 rem side and 0.25 rem top paddings.
        let (inset_x, inset_y) = if grid { (crate::ui::scale::logical(8.0), crate::ui::scale::logical(4.0)) } else { (0.0, 0.0) };
        let origin_y = top + scroll_y + inset_y;
        let mut covered = HashSet::new();
        for position in 0..self.shown.len() {
            let (row, column) = (position / columns, position % columns);
            let row_top = origin_y + row as f32 * row_height;
            let row_bottom = row_top + row_height - if grid { crate::ui::scale::logical(4.0) } else { 0.0 };
            let card_left = bounds.origin.x.as_f32() + inset_x + column as f32 * geometry.cell_width;
            let card_right = card_left + geometry.cell_width - crate::ui::scale::logical(4.0);
            if row_top < lower && row_bottom > upper && (!grid || (card_left < right && card_right > left)) {
                covered.insert(position);
            }
        }
        let keep_anchor = band.modifiers.control || band.modifiers.shift;
        let marked = rectangle_selection(&band.initial, covered, band.modifiers);
        if self.selection.cover(marked, keep_anchor) {
            cx.notify();
        }
    }

    /// Whether a rectangle is being drawn, which shows no tooltip.
    pub(super) fn drawing_band(&self) -> bool {
        self.band.as_ref().is_some_and(|band| band.moved)
    }

    /// Once the rectangle starts, the tooltip of the entry it was pressed beside goes.
    pub(super) fn hide_tooltip_for_band(&mut self, window: &mut Window, cx: &mut App) {
        if let Some(band) = self.band.as_mut().filter(|band| band.moved && !band.tooltip_gone) {
            band.tooltip_gone = true;
            crate::ui::components::tooltip::hover::dismiss(window, cx);
        }
    }

    /// The rectangle, drawn in the element whose bounds the entries were measured from.
    pub(super) fn render_band(&self, theme: OxydeTheme) -> Option<AnyElement> {
        let band = self.band.as_ref().filter(|band| band.moved)?;
        let origin = self.entries_bounds.get().origin.map(|value| value.as_f32());
        let (start, current) = (band.start.map(|value| value.as_f32()), band.current.map(|value| value.as_f32()));
        // Below the column headers, within the entries' area, while the press scrolled away.
        let header = if self.options.grid { 0.0 } else { crate::ui::style::column_header_height() };
        let height = self.entries_bounds.get().size.height.as_f32();
        let top = (start.y.min(current.y) - origin.y).clamp(header, height);
        let bottom = (start.y.max(current.y) - origin.y).clamp(header, height);
        Some(div().absolute()
            .left(px(start.x.min(current.x) - origin.x)).top(px(top))
            .w(px((start.x - current.x).abs())).h(px(bottom - top))
            .bg(theme.selected).border(rems(0.0625)).border_color(theme.accent)
            .into_any_element())
    }

    /// Measures the entries' area: the details' header and rows, or the grid.
    pub(super) fn measure_entries(&self) -> impl IntoElement {
        let bounds = self.entries_bounds.clone();
        canvas(move |measured, _, _| bounds.set(measured), |_, _, _, _| {}).absolute().size_full()
    }
}
