//! The settings panel, opened from the title bar's logo: two columns of settings and,
//! beside the row that opened it, one submenu at a time.
mod about;
mod columns;
mod controls;
mod compression;
mod default_format;
mod data_actions;
mod fallback;
mod integration;
mod measure;
mod panel;
mod submenu;

pub(crate) use data_actions::render_settings_confirmation;

use crate::ui::app::OxydeApp;
use crate::ui::components::popup::{self, PopupAnchor};
use gpui::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SettingsPicker {
    Language,
    Date,
    Compression,
    Integration,
    About,
}

/// Where a submenu opens: beside the row that opened it, its first entry level with
/// that row, as GTK's.
fn submenu_anchor(view: &OxydeApp) -> Option<Bounds<Pixels>> {
    let row = view.settings_submenu_bounds?;
    let chrome = crate::ui::scale::rem_pixels(crate::ui::components::select_menu::chrome()) / 2.0;
    Some(Bounds::new(point(row.left(), row.top() - chrome), row.size))
}

/// A move rightward more than up or down.
fn aims_right(from: Point<Pixels>, to: Point<Pixels>) -> bool {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    dx > px(0.0) && dx >= dy.abs()
}

pub fn render_settings_panel(view: &OxydeApp, handle: Entity<OxydeApp>, window: &mut Window, cx: &mut Context<OxydeApp>) -> AnyElement {
    if !popup::supported(window, cx) {
        return fallback::render(view, handle, window, cx);
    }
    // Pinned to the window's left edge, below the title bar, wherever window buttons push the settings button.
    let bounds = view.settings_button_bounds
        .map(|button| Bounds::new(point(px(0.0), button.top()), size(px(0.0), button.size.height)))
        .unwrap_or(Bounds::new(point(px(0.0), px(0.0)), size(px(0.0), px(44.0))));
    popup::show("settings-panel", PopupAnchor::element(bounds).at_window_edge(), handle, window, cx,
        |view| view.settings_open,
        |view, window, cx| render_native_panel(view, cx.entity(), window, cx),
        |view, _, cx| { view.close_settings(); cx.notify(); });
    div().into_any_element()
}

/// The panel in its own surface, which opens the submenu as a surface of its own.
fn render_native_panel(view: &OxydeApp, handle: Entity<OxydeApp>, window: &mut Window, cx: &mut Context<OxydeApp>) -> AnyElement {
    if let (Some(kind), Some(bounds)) = (view.settings_picker, submenu_anchor(view)) {
        popup::show("settings-picker", PopupAnchor::beside(bounds, false), handle.clone(), window, cx,
            move |view| view.settings_open && view.settings_picker == Some(kind),
            move |view, window, cx| submenu::render(view, cx.entity(), kind, window, cx).into_any_element(),
            |view, _, cx| { view.close_settings_submenu(); cx.notify(); });
    }
    panel::render(view, handle, window, cx).into_any_element()
}

#[cfg(test)]
#[path = "settings/tests/aim.rs"]
mod aim;
