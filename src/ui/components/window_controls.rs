//! Minimize, maximize and close buttons of Oxyde's own title bars, drawn in the
//! style of the running desktop. The desktop chooses which buttons appear, their
//! order and their side.

use oxyde_lib::platform::WindowControlStyle;
use crate::ui::theme::ThemeContext;
use gpui::prelude::FluentBuilder;
use gpui::*;

/// Caption symbols use a 12-unit grid and a one-unit stroke, independently of the
/// Lucide assets and their global stroke setting. Adwaita has bolder symbols on a
/// 16-unit grid; Breeze borrows Lucide chevrons and diamond.
fn icon(style: WindowControlStyle, button: WindowButton, is_maximized: bool) -> &'static str {
    match (style, button) {
        (WindowControlStyle::Adwaita, WindowButton::Close) => "visuals/window-controls/adwaita/close.svg",
        (WindowControlStyle::Adwaita, WindowButton::Minimize) => "visuals/window-controls/adwaita/minimize.svg",
        (WindowControlStyle::Adwaita, WindowButton::Maximize) if is_maximized => "visuals/window-controls/adwaita/restore.svg",
        (WindowControlStyle::Adwaita, WindowButton::Maximize) => "visuals/window-controls/adwaita/maximize.svg",
        (_, WindowButton::Close) => "visuals/window-controls/close.svg",
        (WindowControlStyle::Breeze, WindowButton::Minimize) => "icons/chevron-down.svg",
        (WindowControlStyle::Breeze, WindowButton::Maximize) if is_maximized => "icons/diamond.svg",
        (WindowControlStyle::Breeze, WindowButton::Maximize) => "icons/chevron-up.svg",
        (_, WindowButton::Minimize) => "visuals/window-controls/minimize.svg",
        (_, WindowButton::Maximize) if is_maximized => "visuals/window-controls/restore.svg",
        (_, WindowButton::Maximize) => "visuals/window-controls/maximize.svg",
    }
}

/// Side of the title bar a group of controls sits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Side {
    Left,
    Right,
}

/// The buttons the desktop asks for on `side`, in its order. Without a desktop
/// preference, the usual three on the right.
fn buttons(layout: Option<WindowButtonLayout>, side: Side, allow_resize: bool) -> Vec<WindowButton> {
    let layout = layout.unwrap_or(WindowButtonLayout {
        left: [None; MAX_BUTTONS_PER_SIDE],
        right: [Some(WindowButton::Minimize), Some(WindowButton::Maximize), Some(WindowButton::Close)],
    });
    let group = match side {
        Side::Left => layout.left,
        Side::Right => layout.right,
    };
    group
        .into_iter()
        .flatten()
        .filter(|button| allow_resize || *button == WindowButton::Close)
        .collect()
}

/// Close action shared by both sides of one title bar.
pub(crate) type OnClose = std::rc::Rc<dyn Fn(&mut Window, &mut App)>;

/// Renders the controls of one side. `corner` rounds the outer top edge of a
/// full-height outermost button; `muted` greys the symbols of an inactive window, as
/// Windows does. The caller owns its close lifecycle.
pub(crate) fn render(
    side: Side,
    is_maximized: bool,
    allow_resize: bool,
    corner: Pixels,
    muted: bool,
    cx: &App,
    on_close: OnClose,
) -> impl IntoElement {
    let style = oxyde_lib::platform::platform().window_control_style();
    let buttons = buttons(cx.button_layout(), side, allow_resize);
    let outermost = match side {
        Side::Left => buttons.first().copied(),
        Side::Right => buttons.last().copied(),
    };
    let any = !buttons.is_empty();
    let elements: Vec<AnyElement> = buttons
        .into_iter()
        .map(|button| {
            let symbol = glyph(style, button, is_maximized, muted, cx);
            let shell = match style {
                WindowControlStyle::Windows => {
                    windows_button(button, side, outermost == Some(button), corner, cx).child(symbol)
                }
                WindowControlStyle::Adwaita => hit_zone(button, ADWAITA_DISC, ADWAITA_GAP).child(adwaita_disc(button, cx).child(symbol)),
                WindowControlStyle::Breeze => hit_zone(button, BREEZE_DISC, BREEZE_GAP).child(breeze_disc(button, cx).child(symbol)),
            };
            let on_close = on_close.clone();
            shell
                .window_control_area(match button {
                    WindowButton::Minimize => WindowControlArea::Min,
                    WindowButton::Maximize => WindowControlArea::Max,
                    WindowButton::Close => WindowControlArea::Close,
                })
                .on_mouse_down(MouseButton::Left, move |_, window, cx| match button {
                    WindowButton::Minimize => window.minimize_window(),
                    WindowButton::Maximize => window.zoom_window(),
                    WindowButton::Close => on_close(window, cx),
                })
                .into_any_element()
        })
        .collect();

    // Each hit zone already holds half the gap on each side of its disc.
    let gap = if style == WindowControlStyle::Adwaita { ADWAITA_GAP } else { BREEZE_GAP };
    let edge = (crate::ui::scale::rem_pixels(rems(0.5)) - gap_pixels(gap) / 2.0).round();
    div()
        .h_full()
        .flex()
        .items_center()
        .flex_none()
        .when(any && style != WindowControlStyle::Windows, |row| match side {
            Side::Left => row.pl(edge).pr(edge),
            Side::Right => row.pr(edge),
        })
        .children(elements)
}

const ADWAITA_DISC: Rems = rems(1.5);
const BREEZE_DISC: Rems = rems(1.375);
const ADWAITA_GAP: Rems = rems(0.75);
const BREEZE_GAP: Rems = rems(0.0625);

/// Disc size in an even number of pixels: centered in an even title bar, its edges
/// and center fall on whole pixels instead of being snapped half a pixel aside.
fn disc_pixels(disc: Rems) -> Pixels {
    (crate::ui::scale::rem_pixels(disc) / 2.0).round() * 2.0
}

/// Gap between discs in an even number of pixels: each hit zone holds half of it on
/// each side, and an odd zone would center its even disc half a pixel aside.
fn gap_pixels(gap: Rems) -> Pixels {
    (crate::ui::scale::rem_pixels(gap) / 2.0).round() * 2.0
}

/// Width of a hit zone: its disc and half the gap on each side.
fn zone_pixels(disc: Rems, gap: Rems) -> Pixels {
    disc_pixels(disc) + gap_pixels(gap)
}

/// Glyph size leaving the same whole margin on each side of its disc: an odd
/// difference would put the glyph half a pixel off, snapped toward the top left.
fn centered(disc: Rems, glyph: Rems) -> Pixels {
    let disc = disc_pixels(disc);
    let margin = ((disc - crate::ui::scale::rem_pixels(glyph)) / 2.0).round();
    disc - margin * 2.0
}

fn glyph(style: WindowControlStyle, button: WindowButton, is_maximized: bool, muted: bool, cx: &App) -> Svg {
    let theme = *cx.theme();
    let path = icon(style, button, is_maximized);
    let symbol = svg().path(path).text_color(if muted { theme.text_muted } else { theme.text });
    match style {
        WindowControlStyle::Windows => symbol.size_3().when(button == WindowButton::Close, |symbol| {
            symbol.group_hover("window-close", move |s| s.text_color(theme.text_on_danger))
        }),
        WindowControlStyle::Adwaita => symbol.size(centered(ADWAITA_DISC, rems(1.0))),
        WindowControlStyle::Breeze if button == WindowButton::Close => symbol
            .size(centered(BREEZE_DISC, rems(0.5625)))
            .group_hover("window-close", move |s| s.text_color(theme.text_on_danger)),
        // Sized after Plasma's own title bar: chevrons, diamond and cross read alike.
        WindowControlStyle::Breeze if is_maximized && button == WindowButton::Maximize => symbol.size(centered(BREEZE_DISC, rems(0.75))),
        WindowControlStyle::Breeze => symbol.size(centered(BREEZE_DISC, rems(1.0))),
    }
}

fn windows_button(button: WindowButton, side: Side, outermost: bool, corner: Pixels, cx: &App) -> Stateful<Div> {
    let theme = *cx.theme();
    div()
        .id(button.id())
        .when(button == WindowButton::Close, |shell| shell.group("window-close"))
        .w(rems(2.875))
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .flex_none()
        .when(outermost, |shell| match side {
            Side::Left => shell.rounded_tl(corner),
            Side::Right => shell.rounded_tr(corner),
        })
        .hover(move |s| {
            s.bg(if button == WindowButton::Close { theme.danger_hover } else { theme.control_hover })
        })
}

fn group_name(button: WindowButton) -> &'static str {
    match button {
        WindowButton::Close => "window-close",
        WindowButton::Minimize => "window-minimize",
        WindowButton::Maximize => "window-maximize",
    }
}

/// Full title bar height, the disc plus half the gap on each side: neighbouring
/// zones touch, so every button gets the same width and nothing is dead space.
fn hit_zone(button: WindowButton, disc: Rems, gap: Rems) -> Stateful<Div> {
    div()
        .id(button.id())
        .group(group_name(button))
        .w(zone_pixels(disc, gap))
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .flex_none()
}

/// A disc, faintly filled at rest, darker under the pointer and pressed.
fn adwaita_disc(button: WindowButton, cx: &App) -> Stateful<Div> {
    let theme = *cx.theme();
    let group = group_name(button);
    disc(ADWAITA_DISC)
        .bg(theme.text.opacity(0.1))
        .group_hover(group, move |s| s.bg(theme.text.opacity(0.15)))
        .group_active(group, move |s| s.bg(theme.text.opacity(0.25)))
}

/// A bare glyph; a disc appears under the pointer, red for close.
fn breeze_disc(button: WindowButton, cx: &App) -> Stateful<Div> {
    let theme = *cx.theme();
    let group = group_name(button);
    let close = button == WindowButton::Close;
    disc(BREEZE_DISC)
        .group_hover(group, move |s| s.bg(if close { theme.danger_hover } else { theme.text.opacity(0.15) }))
        .group_active(group, move |s| s.bg(if close { theme.danger_background } else { theme.text.opacity(0.25) }))
}

fn disc(size: Rems) -> Stateful<Div> {
    div()
        .id("disc")
        .size(disc_pixels(size))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .flex_none()
}

#[cfg(test)]
mod tests;
