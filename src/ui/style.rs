use crate::ui::scale::{logical, rem_pixels};
use gpui::*;
use lazy_static::lazy_static;
use std::sync::Arc;

lazy_static! {
    pub static ref TNUM: FontFeatures = FontFeatures(Arc::new(vec![("tnum".to_string(), 1)]));
}

pub const DEFAULT_FONT_WEIGHT: FontWeight = FontWeight::NORMAL;

/// GPUI's shadow presets use pixels; retain their colors and proportions while
/// scaling their geometry alongside the rest of the interface.
pub fn rem_shadow(large: bool) -> Vec<BoxShadow> {
    let mut preset = if large {
        div().shadow_lg()
    } else {
        div().shadow_md()
    };
    let factor = logical(1.0);
    let mut shadows = preset.style().box_shadow.take().unwrap_or_default();
    for shadow in &mut shadows {
        shadow.offset.x *= factor;
        shadow.offset.y *= factor;
        shadow.blur_radius *= factor;
        shadow.spread_radius *= factor;
    }
    shadows
}

// Pane chrome follows the heights of the Tauri interface.
pub const TAB_BAR_HEIGHT: Rems = rems(2.0);
pub const NAV_BAR_HEIGHT: Rems = rems(2.25);
pub const STATUS_BAR_HEIGHT: Rems = rems(2.0);
pub const COLUMN_HEADER_HEIGHT: Rems = rems(1.625);
pub fn column_header_height() -> f32 {
    rem_pixels(COLUMN_HEADER_HEIGHT).as_f32()
}

pub const ROW_HEIGHT: Rems = rems(1.75);
pub fn row_height() -> f32 {
    rem_pixels(ROW_HEIGHT).as_f32()
}
pub const GRID_CARD_MIN_WIDTH: Rems = rems(7.25);
pub fn grid_card_min_width() -> f32 {
    rem_pixels(GRID_CARD_MIN_WIDTH).as_f32()
}
pub const GRID_CARD_ROW_HEIGHT: Rems = rems(8.5);
pub fn grid_card_row_height() -> f32 {
    rem_pixels(GRID_CARD_ROW_HEIGHT).as_f32()
}

pub const DETAILS_HORIZONTAL_PADDING: Rems = rems(1.75);
pub fn details_horizontal_padding() -> f32 {
    rem_pixels(DETAILS_HORIZONTAL_PADDING).as_f32()
}

pub fn menu_list_max_height(viewport: f32, header: bool, bottom_padding: f32, maximum: f32) -> f32 {
    let chrome = logical(if header { 42.0 } else { 6.0 }) + logical(bottom_padding);
    maximum.min((viewport - chrome - logical(16.0)).max(logical(26.0)))
}

/// Gives file names priority over metadata while keeping every column usable.
pub struct GridGeometry {
    pub columns: usize,
    pub content_width: f32,
    pub cell_width: f32,
}

impl GridGeometry {
    pub fn for_viewport(viewport: f32, scrollbar_width: f32) -> Self {
        let columns = ((viewport - logical(20.0)) / grid_card_min_width())
            .floor()
            .max(1.0) as usize;
        let content_width =
            (viewport - logical(20.0) - scrollbar_width).max(grid_card_min_width() + logical(4.0));
        Self {
            columns,
            content_width,
            cell_width: content_width / columns as f32,
        }
    }
}

/// Column reservations are kept at the baseline scale in TabState so they do
/// not depend on whether a background file load finishes before or after zoom.
pub struct DetailsColumnWidths {
    pub name: f32,
    pub location: f32,
    pub file_type: f32,
    pub size: f32,
    pub date: f32,
}

impl DetailsColumnWidths {
    pub fn content_width(&self) -> f32 {
        self.total() + details_horizontal_padding()
    }

    pub fn for_viewport_with_search(viewport: f32, file_type: f32, size: f32, date: f32, search: bool) -> Self {
        if !search { return Self::for_viewport(viewport, file_type, size, date); }
        let location_min = logical(180.0);
        let name_min = logical(220.0);
        let mut columns = Self::for_viewport(viewport - location_min, file_type, size, date);
        // Name and location share the space remaining after the metadata columns.
        // Keep their minima in narrow panes; only that case needs horizontal scrolling.
        let extra = (columns.name - name_min).max(0.0);
        columns.location = location_min + extra * 0.45;
        columns.name -= extra * 0.45;
        columns
    }

    pub fn for_viewport(viewport: f32, file_type: f32, size: f32, date: f32) -> Self {
        let name_min = logical(220.0);
        let available = (viewport - details_horizontal_padding()).max(0.0);
        let name_target = name_min.max(available * 0.4);
        // Sizes and dates keep their measured width, even in narrow panes.
        let minimums = [logical(64.0), logical(size).max(logical(64.0)), logical(date).max(logical(64.0))];
        let preferred = [
            logical(file_type).max(minimums[0]),
            logical(size).max(minimums[1]),
            logical(date).max(minimums[2]),
        ];
        let preferred_total: f32 = preferred.iter().sum();
        let shrinkable: f32 = preferred
            .iter()
            .zip(minimums)
            .map(|(width, min)| width - min)
            .sum();
        let deficit = (preferred_total - (available - name_target)).max(0.0);
        let shrink_ratio = if shrinkable > 0.0 {
            (deficit / shrinkable).min(1.0)
        } else {
            0.0
        };
        let widths = std::array::from_fn::<_, 3, _>(|index| {
            preferred[index] - (preferred[index] - minimums[index]) * shrink_ratio
        });
        Self {
            name: (available - widths.iter().sum::<f32>()).max(name_min),
            location: 0.0,
            file_type: widths[0],
            size: widths[1],
            date: widths[2],
        }
    }

    pub fn total(&self) -> f32 {
        self.name + self.location + self.file_type + self.size + self.date
    }
}

/// Baseline width: row padding, or header padding plus its sort/filter indicators.
pub(crate) fn metadata_column_width(content: f32, header: f32, filtered: bool) -> f32 {
    (content + 16.0).max(header + 36.0 + if filtered { 12.0 } else { 0.0 }).ceil()
}

pub fn h_flex() -> Div {
    div().flex().flex_row().items_center()
}

#[cfg(test)]
mod tests {
    use super::{grid_card_row_height, logical, row_height, DetailsColumnWidths, GridGeometry};
    use crate::ui::scale::set_reference;

    #[test]
    fn search_columns_resize_together_and_scroll_only_below_their_minima() {
        set_reference(16);
        for reference in [16, 24, 32] {
            set_reference(reference);
            let mut previous = None;
            for width in [400.0, 800.0, 1000.0, 1400.0] {
                let viewport = logical(width);
                let columns = DetailsColumnWidths::for_viewport_with_search(viewport, 100.0, 100.0, 180.0, true);
                assert!(columns.name >= logical(220.0));
                assert!(columns.location >= logical(180.0));
                assert!(columns.file_type >= logical(64.0));
                assert_eq!(columns.size, logical(100.0));
                assert_eq!(columns.date, logical(180.0));
                if width >= 800.0 {
                    assert!((columns.content_width() - viewport).abs() < 0.01);
                } else {
                    let max_offset = columns.content_width() - viewport;
                    assert!(max_offset > logical(120.0));
                    assert_eq!(logical(120.0).min(max_offset), logical(120.0));
                }
                if let Some((name, location)) = previous {
                    assert!(columns.name >= name && columns.location >= location);
                }
                previous = Some((columns.name, columns.location));
            }
        }
        set_reference(16);
    }

    #[test]
    fn measured_metadata_widths_reserve_padding_and_header_indicators_only() {
        use super::metadata_column_width;
        assert_eq!(metadata_column_width(48.0, 28.0, false), 64.0);
        assert_eq!(metadata_column_width(48.0, 28.0, true), 76.0);
        assert_eq!(metadata_column_width(136.0, 75.0, false), 152.0);
        assert_eq!(metadata_column_width(190.0, 28.0, false), 206.0);
    }

    #[test]
    fn grid_and_detail_geometry_follow_the_reference_without_double_scaling() {
        set_reference(16);
        let base_grid = GridGeometry::for_viewport(1000.0, logical(12.0));
        let base_columns = DetailsColumnWidths::for_viewport(600.0, 140.0, 100.0, 180.0);
        for reference in [10, 20, 24, 32] {
            set_reference(reference);
            let factor = reference as f32 / 16.0;
            let grid = GridGeometry::for_viewport(1000.0 * factor, logical(12.0));
            assert_eq!(grid.columns, base_grid.columns);
            assert!((grid.cell_width - base_grid.cell_width * factor).abs() < 0.01);
            let columns = DetailsColumnWidths::for_viewport(600.0 * factor, 140.0, 100.0, 180.0);
            assert!((columns.total() - base_columns.total() * factor).abs() < 0.01);
            assert!((columns.name - base_columns.name * factor).abs() < 0.01);
            assert_eq!(row_height(), 28.0 * factor);
            assert_eq!(grid_card_row_height(), 136.0 * factor);
        }
        set_reference(16);
    }
}

#[cfg(test)]
#[path = "style/tests/shadows.rs"]
mod shadow_tests;

#[cfg(test)]
#[path = "style/tests/size_column.rs"]
mod size_column_tests;
