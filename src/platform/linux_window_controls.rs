//! Chooses the window-button look from the running desktop.

use super::WindowControlStyle;

/// Read once: the desktop does not change during a session.
pub(super) fn style() -> WindowControlStyle {
    static STYLE: std::sync::OnceLock<WindowControlStyle> = std::sync::OnceLock::new();
    *STYLE.get_or_init(|| for_desktop(&std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default()))
}

/// KDE gets Breeze; GNOME and every other desktop get Adwaita, as GTK applications do.
fn for_desktop(desktop: &str) -> WindowControlStyle {
    if desktop.split(':').any(|part| part.eq_ignore_ascii_case("KDE")) {
        WindowControlStyle::Breeze
    } else {
        WindowControlStyle::Adwaita
    }
}

#[cfg(test)]
mod tests;
