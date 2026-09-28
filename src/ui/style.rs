use gpui::*;
use lazy_static::lazy_static;
use std::sync::Arc;

lazy_static! {
    pub static ref TNUM: FontFeatures = FontFeatures(Arc::new(vec![("tnum".to_string(), 1)]));
}

pub const ROW_HEIGHT: f32 = 28.0;

pub fn h_flex() -> Div {
    div().flex().flex_row().items_center()
}

#[allow(dead_code)]
pub fn v_flex() -> Div {
    div().flex().flex_col()
}
