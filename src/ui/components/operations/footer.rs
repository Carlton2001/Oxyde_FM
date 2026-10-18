use crate::ui::components::tooltip::NativeTooltipExt;
use crate::ui::app::OxydeApp;
use crate::ui::components::tooltip::Tooltip;
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;

pub fn render_operations_footer(
    app: &OxydeApp,
    handle: Entity<OxydeApp>,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let theme = *cx.theme();
    let (total, active) = app.file_ops.operation_counts();
    let enabled = total > 0;
    let tooltip = cx.i18n().text(Message::OpsShowPanel);
    div()
        .id("title-operations")
        .w(rems(2.25))
        .h_full()
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .gap(rems(0.375))
        .px(rems(0.5))
        .rounded_sm()
        .text_color(theme.text_muted)
        .occlude()
        .native_tooltip(Tooltip::text(tooltip))
        .when(!enabled, |button| button.opacity(0.4))
        .when(enabled, |button| {
            button.hover(|style| style.bg(theme.control_hover).text_color(theme.text))
        })
        .child({
            let icon = div().size_4().rounded_full().child(svg()
                .path("icons/download.svg")
                .text_color(if active > 0 { theme.accent } else { theme.text })
                .size_4());
            let halo = move |breath: f32, strength: f32| vec![BoxShadow {
                inset: false,
                color: theme.accent.opacity((0.2 + 0.5 * breath) * strength),
                offset: point(px(0.0), px(0.0)),
                blur_radius: crate::ui::scale::rem_pixels(rems(0.375 + 0.375 * breath)),
                spread_radius: crate::ui::scale::rem_pixels(rems(0.0625 + 0.125 * breath)),
            }];
            // The shared glow spring lets the halo fade out when the last operation ends.
            let glow = crate::ui::elements::motion::linger(active > 0);
            if active > 0 {
                icon.with_animation(
                    "operations-breathing",
                    Animation::new(std::time::Duration::from_millis(2200)).repeat(),
                    move |icon, delta| icon.shadow(halo((1.0 - (delta * std::f32::consts::TAU).cos()) * 0.5, 1.0)),
                )
                .with_spring("operations-glow", glow, |icon, _| icon)
                .into_any_element()
            } else {
                icon.with_spring("operations-glow", glow, move |icon, strength| {
                    icon.when(strength > 0.0, |icon| icon.shadow(halo(0.5, strength)))
                })
                .into_any_element()
            }
        })
        .when(enabled, |button| {
            button.on_mouse_down(MouseButton::Left, move |_, _, cx| {
                super::open_operations_window(handle.clone(), cx);
            })
        })
}
