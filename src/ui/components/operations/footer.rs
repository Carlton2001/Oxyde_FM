use crate::ui::components::tooltip::NativeTooltipExt;
use crate::ui::app::OxydeApp;
use crate::ui::components::tooltip::Tooltip;
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::style::TabularFigures;
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
        .occlude()
        .fixed_tooltip(Tooltip::text(tooltip))
        // Square fill and disabled icon color, as the other title bar buttons.
        .when(enabled, |button| button.hover(|style| style.bg(theme.control_hover)))
        // A static mark: an animation would redraw the whole window at every frame.
        .child(div().relative().size_4()
            .child(svg()
                .path("icons/download.svg")
                .text_color(if !enabled { theme.icon_disabled } else if active > 0 { theme.accent } else { theme.text })
                .size_4())
            .when(active > 0, |icon| icon.child(count_badge(active, theme))))
        .when(enabled, |button| {
            button.on_mouse_down(MouseButton::Left, move |_, _, cx| {
                super::open_operations_window(handle.clone(), None, cx);
            })
        })
}

/// The number of running operations, on the icon's corner.
fn count_badge(active: usize, theme: crate::ui::theme::OxydeTheme) -> Div {
    div()
        .absolute()
        .top(rems(-0.375))
        .left(rems(0.625))
        .h(rems(0.875))
        .min_w(rems(0.875))
        .px(rems(0.1875))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(theme.accent)
        .text_color(theme.text_on_accent)
        .text_size(rems(0.5625))
        .line_height(rems(0.875))
        .font_weight(FontWeight::SEMIBOLD)
        .tabular_figures()
        .child(if active > 99 { "99+".to_string() } else { active.to_string() })
}
