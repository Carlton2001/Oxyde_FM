//! Rem conversion at the boundary between layout and measured GPUI geometry.
//! Measurements returned by GPUI (pointer positions, bounds and scroll offsets)
//! are already pixels and must never be scaled a second time.
use gpui::{px, Pixels, Rems};
use std::cell::Cell;

pub const BASE_REM_SIZE: u32 = 16;
pub const MIN_REM_SIZE: u32 = 10;
pub const MAX_REM_SIZE: u32 = 32;

thread_local! {
    // All GPUI layout and pointer handling runs on the UI thread. This reference
    // is synchronized with Window::rem_size before rendering and when zooming.
    static REM_SIZE: Cell<u32> = const { Cell::new(BASE_REM_SIZE) };
}

pub fn normalized_reference(value: u32) -> u32 {
    if value == 0 {
        BASE_REM_SIZE
    } else {
        value.clamp(MIN_REM_SIZE, MAX_REM_SIZE)
    }
}

pub fn set_reference(value: u32) {
    REM_SIZE.with(|size| size.set(normalized_reference(value)));
}

/// Resolve a rem length for APIs that require concrete pixels (text shaping,
/// canvas, scrolling, virtual lists and pointer hit testing).
pub fn rem_pixels(value: Rems) -> Pixels {
    REM_SIZE.with(|size| value.to_pixels(px(size.get() as f32)))
}

/// Convert a baseline measurement at 16px/rem to current physical layout pixels.
/// Use only for design dimensions, never measured coordinates or ratios.
pub fn logical(value: f32) -> f32 {
    rem_pixels(gpui::rems(value / BASE_REM_SIZE as f32)).as_f32()
}

/// A design length GPUI will lay out unchanged. GPUI rounds each authored length to device
/// pixels, so a total computed from unrounded rows can differ from the rows it must contain.
pub fn device_snapped(value: f32, scale_factor: f32) -> f32 {
    (value * scale_factor - 0.5).ceil().max(0.0) / scale_factor
}

#[cfg(test)]
#[path = "scale/tests/device_pixels.rs"]
mod device_pixel_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rem_reference_scales_geometry_and_validates_preferences() {
        assert_eq!(normalized_reference(0), 16);
        assert_eq!(normalized_reference(1), 10);
        assert_eq!(normalized_reference(100), 32);
        set_reference(16);
        assert_eq!(logical(32.0), 32.0);
        set_reference(24);
        assert_eq!(logical(32.0), 48.0);
        assert_eq!(rem_pixels(gpui::rems(0.5)), px(12.0));
        set_reference(16);
    }
}
