//! Saturation/value field and hue strip for choosing a color by eye or with the arrows.
//! The caller owns the color, the focus, the pointer capture and what a change does.

use crate::ui::elements::focus_ring::{around, ring};
use crate::ui::theme::OxydeTheme;
use gpui::prelude::*;
use gpui::*;
use std::{cell::Cell, rc::Rc};

#[cfg(test)]
#[path = "color_picker/tests/conversion.rs"]
mod conversion;
#[cfg(test)]
#[path = "color_picker/tests/keys.rs"]
mod keys;

/// Hue in degrees [0, 360], 360 being red again at the strip's right end;
/// saturation and value in [0, 1].
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

const RADIUS: f32 = 0.375;
const FIELD_KNOB: f32 = 0.875;
const HUE_KNOB: f32 = 1.0;
const HUE_HEIGHT: f32 = 0.75;
/// Arrow step, as a fraction of the axis; Shift takes ten.
const STEP: f32 = 0.01;

fn measure(target: MeasuredBounds) -> impl IntoElement {
    canvas(|_, _, _| (), move |bounds, _, _, _| target.set(Some(bounds))).absolute().size_full()
}

/// Where the knob's center travels: inset by half a knob, so the knob stays inside
/// at both ends as a slider's. The pointer is measured against the same box.
fn travel(knob: f32, bounds: MeasuredBounds) -> Div {
    let inset = rems(knob / 2.0);
    div().absolute().top(inset).bottom(inset).left(inset).right(inset).child(measure(bounds))
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
    fraction(position.x, bounds.left(), bounds.size.width) * 360.0
}

fn step(shift: bool) -> f32 {
    if shift { STEP * 10.0 } else { STEP }
}

/// The field's color after `key`: Left and Right move saturation, Up and Down value.
pub(crate) fn field_key(hsv: Hsv, key: &str, shift: bool) -> Option<Hsv> {
    let step = step(shift);
    let (s, v) = match key {
        "left" => (hsv.s - step, hsv.v),
        "right" => (hsv.s + step, hsv.v),
        "up" => (hsv.s, hsv.v + step),
        "down" => (hsv.s, hsv.v - step),
        _ => return None,
    };
    Some(Hsv { s: s.clamp(0.0, 1.0), v: v.clamp(0.0, 1.0), ..hsv })
}

/// The strip's hue after `key`: Left/Down and Right/Up move it, Home and End go to the ends.
pub(crate) fn hue_key(h: f32, key: &str, shift: bool) -> Option<f32> {
    let step = step(shift) * 360.0;
    let h = match key {
        "left" | "down" => h - step,
        "right" | "up" => h + step,
        "home" => 0.0,
        "end" => 360.0,
        _ => return None,
    };
    Some(h.clamp(0.0, 360.0))
}

fn knob(size: f32, color: Hsla) -> Div {
    let size = rems(size);
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

/// Shared by both parts: a focus stop that takes the keyboard on press, the accent ring
/// only when the keyboard brought it, and `on_key` for the arrows.
fn control(
    id: impl Into<ElementId>,
    focus: &FocusHandle,
    window: &Window,
    theme: OxydeTheme,
    on_press: impl Fn(Point<Pixels>, &mut Window, &mut App) + 'static,
    on_key: impl Fn(&str, bool, &mut Window, &mut App) -> bool + 'static,
) -> (Stateful<Div>, bool) {
    let ring = crate::ui::elements::focus_ring::keyboard_focused(focus, window);
    let press_focus = focus.clone();
    let element = div()
        .id(id)
        .relative()
        .rounded(rems(RADIUS))
        .border(rems(0.0625))
        .border_color(theme.border)
        .track_focus(focus)
        .on_mouse_down(MouseButton::Left, move |event, window, cx| {
            cx.stop_propagation();
            window.focus(&press_focus, cx);
            on_press(event.position, window, cx);
        })
        .on_key_down(move |event, window, cx| {
            let modifiers = event.keystroke.modifiers;
            if !(modifiers.control || modifiers.alt || modifiers.platform)
                && on_key(event.keystroke.key.as_str(), modifiers.shift, window, cx)
            {
                cx.stop_propagation();
            }
        });
    (element, ring)
}

/// The saturation (left to right) and value (top to bottom) square for `hsv.h`.
/// `on_key` receives the new color when an arrow moves it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn color_field(
    id: impl Into<ElementId>,
    hsv: Hsv,
    bounds: MeasuredBounds,
    focus: &FocusHandle,
    window: &Window,
    theme: OxydeTheme,
    on_press: impl Fn(Point<Pixels>, &mut Window, &mut App) + 'static,
    on_key: impl Fn(Hsv, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let transparent_white = gpui::white().opacity(0.0);
    let transparent_black = gpui::black().opacity(0.0);
    let (field, ring_visible) = control(id, focus, window, theme, on_press, move |key, shift, window, cx| {
        field_key(hsv, key, shift).map(|next| on_key(next, window, cx)).is_some()
    });
    field
        .bg(pure_hue(hsv.h))
        .child(div().absolute().inset_0().rounded(rems(RADIUS))
            .bg(linear_gradient(90.0, linear_color_stop(gpui::white(), 0.0), linear_color_stop(transparent_white, 1.0))))
        .child(div().absolute().inset_0().rounded(rems(RADIUS))
            .bg(linear_gradient(180.0, linear_color_stop(transparent_black, 0.0), linear_color_stop(gpui::black(), 1.0))))
        .child(travel(FIELD_KNOB, bounds)
            .child(knob(FIELD_KNOB, rgb(hsv.to_rgb()).into()).left(relative(hsv.s)).top(relative(1.0 - hsv.v))))
        .when(ring_visible, |field| field.child(ring(theme).rounded(around(rems(RADIUS)))))
}

/// The hue strip, red to red from left to right. `on_key` receives the new hue.
#[allow(clippy::too_many_arguments)]
pub(crate) fn hue_strip(
    id: impl Into<ElementId>,
    h: f32,
    bounds: MeasuredBounds,
    focus: &FocusHandle,
    window: &Window,
    theme: OxydeTheme,
    on_press: impl Fn(Point<Pixels>, &mut Window, &mut App) + 'static,
    on_key: impl Fn(f32, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let segments = (0..6).map(|index| {
        let from = index as f32 * 60.0;
        div().flex_1().h_full()
            .when(index == 0, |segment| segment.rounded_l(rems(RADIUS)))
            .when(index == 5, |segment| segment.rounded_r(rems(RADIUS)))
            .bg(linear_gradient(90.0, linear_color_stop(pure_hue(from), 0.0), linear_color_stop(pure_hue(from + 60.0), 1.0)))
    });
    let (strip, ring_visible) = control(id, focus, window, theme, on_press, move |key, shift, window, cx| {
        hue_key(h, key, shift).map(|next| on_key(next, window, cx)).is_some()
    });
    // The knob is taller than the strip: its travel keeps the strip's height, centered.
    strip
        .h(rems(HUE_HEIGHT))
        .flex()
        .children(segments)
        .child(travel(HUE_KNOB, bounds).top_0().bottom_0()
            .child(knob(HUE_KNOB, pure_hue(h)).left(relative(h / 360.0)).top(relative(0.5))))
        .when(ring_visible, |strip| strip.child(ring(theme).rounded(around(rems(RADIUS)))))
}
