//! Saturation/value field and hue strip for choosing a color by eye.
//! The caller owns the color, the pointer capture and what a change does.

use crate::ui::theme::OxydeTheme;
use gpui::prelude::*;
use gpui::*;
use std::{cell::Cell, rc::Rc};

#[cfg(test)]
#[path = "color_picker/tests/conversion.rs"]
mod tests;

/// Hue in degrees [0, 360), saturation and value in [0, 1].
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Hsv {
    pub h: f32,
    pub s: f32,
    pub v: f32,
}

impl Hsv {
    pub(crate) fn from_rgb(color: u32) -> Self {
        let channel = |shift: u32| ((color >> shift) & 0xff) as f32 / 255.0;
        let (r, g, b) = (channel(16), channel(8), channel(0));
        let max = r.max(g).max(b);
        let delta = max - r.min(g).min(b);
        let h = if delta == 0.0 {
            0.0
        } else if max == r {
            60.0 * ((g - b) / delta).rem_euclid(6.0)
        } else if max == g {
            60.0 * ((b - r) / delta + 2.0)
        } else {
            60.0 * ((r - g) / delta + 4.0)
        };
        Self { h, s: if max == 0.0 { 0.0 } else { delta / max }, v: max }
    }

    pub(crate) fn to_rgb(self) -> u32 {
        let c = self.v * self.s;
        let sector = (self.h.rem_euclid(360.0)) / 60.0;
        let x = c * (1.0 - (sector % 2.0 - 1.0).abs());
        let (r, g, b) = match sector as u32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        let m = self.v - c;
        let byte = |v: f32| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u32;
        byte(r) << 16 | byte(g) << 8 | byte(b)
    }
}

fn pure_hue(h: f32) -> Hsla {
    rgb(Hsv { h, s: 1.0, v: 1.0 }.to_rgb()).into()
}

/// Where the pointer was last measured; filled at paint, read by the caller's drag.
pub(crate) type MeasuredBounds = Rc<Cell<Option<Bounds<Pixels>>>>;

fn measure(target: MeasuredBounds) -> impl IntoElement {
    canvas(|_, _, _| (), move |bounds, _, _, _| target.set(Some(bounds))).absolute().size_full()
}

fn fraction(value: Pixels, start: Pixels, length: Pixels) -> f32 {
    if length <= px(0.0) { 0.0 } else { ((value - start) / length).clamp(0.0, 1.0) }
}

/// Saturation and value under `position`, for a field at `bounds`.
pub(crate) fn field_value(bounds: Bounds<Pixels>, position: Point<Pixels>, h: f32) -> Hsv {
    Hsv {
        h,
        s: fraction(position.x, bounds.left(), bounds.size.width),
        v: 1.0 - fraction(position.y, bounds.top(), bounds.size.height),
    }
}

/// Hue under `position`, for a strip at `bounds`.
pub(crate) fn hue_value(bounds: Bounds<Pixels>, position: Point<Pixels>) -> f32 {
    fraction(position.x, bounds.left(), bounds.size.width) * 359.0
}

fn knob(color: Hsla) -> Div {
    let size = rems(0.875);
    div()
        .absolute()
        .size(size)
        .ml(-size / 2.0)
        .mt(-size / 2.0)
        .rounded_full()
        .border(rems(0.125))
        .border_color(gpui::white())
        .shadow(vec![BoxShadow {
            color: gpui::black().opacity(0.45),
            offset: point(px(0.0), px(0.0)),
            blur_radius: px(2.0),
            spread_radius: px(0.0),
            inset: false,
        }])
        .bg(color)
}

/// The saturation (left to right) and value (top to bottom) square for `hsv.h`.
pub(crate) fn color_field(
    id: impl Into<ElementId>,
    hsv: Hsv,
    bounds: MeasuredBounds,
    theme: OxydeTheme,
    on_press: impl Fn(Point<Pixels>, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let transparent_white = gpui::white().opacity(0.0);
    let transparent_black = gpui::black().opacity(0.0);
    div()
        .id(id)
        .relative()
        .rounded_md()
        .border(rems(0.0625))
        .border_color(theme.border)
        .bg(pure_hue(hsv.h))
        .child(measure(bounds))
        .child(div().absolute().inset_0().rounded_md()
            .bg(linear_gradient(90.0, linear_color_stop(gpui::white(), 0.0), linear_color_stop(transparent_white, 1.0))))
        .child(div().absolute().inset_0().rounded_md()
            .bg(linear_gradient(180.0, linear_color_stop(transparent_black, 0.0), linear_color_stop(gpui::black(), 1.0))))
        .child(knob(rgb(hsv.to_rgb()).into()).left(relative(hsv.s)).top(relative(1.0 - hsv.v)))
        .on_mouse_down(MouseButton::Left, move |event, window, cx| {
            cx.stop_propagation();
            on_press(event.position, window, cx);
        })
}

/// The hue strip, red to red from left to right.
pub(crate) fn hue_strip(
    id: impl Into<ElementId>,
    h: f32,
    bounds: MeasuredBounds,
    theme: OxydeTheme,
    on_press: impl Fn(Point<Pixels>, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let segments = (0..6).map(|index| {
        let from = index as f32 * 60.0;
        div().flex_1().h_full()
            .when(index == 0, |segment| segment.rounded_l_md())
            .when(index == 5, |segment| segment.rounded_r_md())
            .bg(linear_gradient(90.0, linear_color_stop(pure_hue(from), 0.0), linear_color_stop(pure_hue((from + 60.0) % 360.0), 1.0)))
    });
    div()
        .id(id)
        .relative()
        .h(rems(0.875))
        .flex()
        .rounded_md()
        .border(rems(0.0625))
        .border_color(theme.border)
        .children(segments)
        .child(measure(bounds))
        .child(knob(pure_hue(h)).left(relative(h / 359.0)).top(relative(0.5)))
        .on_mouse_down(MouseButton::Left, move |event, window, cx| {
            cx.stop_propagation();
            on_press(event.position, window, cx);
        })
}
