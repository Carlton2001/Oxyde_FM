use gpui::*;
use lazy_static::lazy_static;
use std::sync::Arc;

lazy_static! {
    pub static ref TNUM: FontFeatures = FontFeatures(Arc::new(vec![("tnum".to_string(), 1)]));
}

pub const ROW_HEIGHT: f32 = 28.0;
pub const GRID_CARD_MIN_WIDTH: f32 = 116.0;
pub const GRID_CARD_ROW_HEIGHT: f32 = 136.0;

pub const DETAILS_HORIZONTAL_PADDING: f32 = 28.0;

/// Gives file names priority over metadata while keeping every column usable.
pub struct DetailsColumnWidths {
    pub name: f32,
    pub file_type: f32,
    pub size: f32,
    pub date: f32,
}

impl DetailsColumnWidths {
    pub fn for_viewport(viewport: f32, file_type: f32, size: f32, date: f32) -> Self {
        const NAME_MIN: f32 = 220.0;
        let available = (viewport - DETAILS_HORIZONTAL_PADDING).max(0.0);
        let name_target = NAME_MIN.max(available * 0.4);
        let minimums = [80.0, 80.0, 150.0];
        let preferred = [
            file_type.max(minimums[0]),
            size.max(minimums[1]),
            date.max(minimums[2]),
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
            name: (available - widths.iter().sum::<f32>()).max(NAME_MIN),
            file_type: widths[0],
            size: widths[1],
            date: widths[2],
        }
    }

    pub fn total(&self) -> f32 {
        self.name + self.file_type + self.size + self.date
    }
}

pub fn h_flex() -> Div {
    div().flex().flex_row().items_center()
}

#[allow(dead_code)]
pub fn v_flex() -> Div {
    div().flex().flex_col()
}
