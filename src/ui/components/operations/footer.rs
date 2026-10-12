use crate::ui::app::OxydeApp;
use crate::ui::components::tooltip::Tooltip;
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;
use super::is_final;

pub fn render_operations_footer(
    app: &OxydeApp,
    handle: Entity<OxydeApp>,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let theme = *cx.theme();
    let operations = app.file_ops.list_operations();
    let active = operations.iter().filter(|op| !is_final(&op.status)).count();
    let tooltip = cx.i18n().text(if app.operations_panel_open {
        Message::OpsHidePanel
    } else {
        Message::OpsShowPanel
    });
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
        .tooltip(Tooltip::text(tooltip))
        .hover(|style| style.bg(theme.control_hover).text_color(theme.text))
        .child({
            let icon = div().size_4().rounded_full().child(svg()
                .path("icons/download.svg")
                .text_color(if active > 0 { theme.accent } else { theme.text_muted })
                .size_4());
            if active > 0 {
                icon.with_animation(
                    "operations-breathing",
                    Animation::new(std::time::Duration::from_millis(2200)).repeat(),
                    move |icon, delta| {
                        let breath = (1.0 - (delta * std::f32::consts::TAU).cos()) * 0.5;
                        icon.shadow(vec![BoxShadow {
                            inset: false,
                            color: theme.accent.opacity(0.2 + 0.5 * breath),
                            offset: point(px(0.0), px(0.0)),
                            blur_radius: crate::ui::scale::rem_pixels(rems(0.375 + 0.375 * breath)),
                            spread_radius: crate::ui::scale::rem_pixels(rems(0.0625 + 0.125 * breath)),
                        }])
                    },
                ).into_any_element()
            } else {
                icon.into_any_element()
            }
        })
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            handle.update(cx, |this, cx| {
                this.operations_panel_open = !this.operations_panel_open;
                this.operations_panel_opened_manually = this.operations_panel_open;
                cx.notify();
            });
        })
}
