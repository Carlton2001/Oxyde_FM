use crate::ui::theme::OxydeTheme;
use gpui::prelude::*;
use gpui::*;

/// Visual role, independent of whether the content is an icon, text, or both.
#[derive(Clone, Copy)]
pub enum ButtonVariant {
    Ghost,
    Secondary,
    Primary,
    Danger,
}

/// Shared visual states. Callers retain geometry, focus routing and actions.
/// Disabled callers must also omit or guard their action handlers.
pub fn button_style(
    button: Stateful<Div>,
    theme: OxydeTheme,
    variant: ButtonVariant,
    enabled: bool,
) -> Stateful<Div> {
    let (background, foreground, hover) = match variant {
        ButtonVariant::Ghost => (transparent_black(), theme.text_muted, theme.control_hover),
        ButtonVariant::Secondary => (theme.control_hover, theme.text, theme.selected),
        ButtonVariant::Primary => (theme.accent, theme.text_on_accent, theme.accent),
        ButtonVariant::Danger => (theme.error, theme.text_on_danger, theme.danger_hover),
    };
    let filled = matches!(variant, ButtonVariant::Primary | ButtonVariant::Danger);
    let secondary = matches!(variant, ButtonVariant::Secondary);
    button
        .bg(background)
        .when(secondary, |d| d.border(rems(0.0625)).border_color(theme.border))
        .text_color(if enabled {
            foreground
        } else {
            theme.icon_disabled
        })
        .when(!enabled && filled, |d| d.opacity(0.5))
        .when(enabled, |d| {
            d.hover(move |s| {
                let s = s.bg(hover);
                let s = if secondary { s.border_color(theme.accent.opacity(0.5)) } else { s };
                if filled {
                    s.opacity(0.85)
                } else {
                    s
                }
            })
            .active(move |s| {
                let s = s.bg(if filled { background } else { theme.selected });
                if filled {
                    s.opacity(0.7)
                } else {
                    s
                }
            })
            // Focus styling is shared; callers retain their existing focus routing.
            .focus_visible(move |s| s.border(rems(0.0625)).border_color(theme.accent))
        })
}
