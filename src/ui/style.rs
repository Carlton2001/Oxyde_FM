use gpui::*;
use lazy_static::lazy_static;
use std::sync::Arc;

lazy_static! {
    pub static ref TNUM: FontFeatures = FontFeatures(Arc::new(vec![("tnum".to_string(), 1)]));
}

pub const ROW_HEIGHT: f32 = 28.0;

pub fn format_size(size: u64, is_dir: bool, is_calculated: bool) -> String {
    if is_dir && !is_calculated {
        return String::new();
    }
    if !is_dir && size == 0 {
        return "0 o".to_string();
    }
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if size >= GB {
        format!("{:.1} Go", size as f64 / GB as f64)
    } else if size >= MB {
        format!("{:.1} Mo", size as f64 / MB as f64)
    } else if size >= KB {
        format!("{:.1} Ko", size as f64 / KB as f64)
    } else {
        format!("{} o", size)
    }
}

pub fn format_date(timestamp_ms: u64) -> String {
    use chrono::{DateTime, Local};
    if timestamp_ms == 0 {
        return "–".to_string();
    }
    DateTime::from_timestamp_millis(timestamp_ms as i64)
        .map(|dt| {
            dt.with_timezone(&Local)
                .format("%d/%m/%Y %H:%M:%S")
                .to_string()
        })
        .unwrap_or_else(|| "–".to_string())
}

pub fn h_flex() -> Div {
    div().flex().flex_row().items_center()
}

#[allow(dead_code)]
pub fn v_flex() -> Div {
    div().flex().flex_col()
}
