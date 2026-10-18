use super::{button_style, ButtonVariant};
use crate::ui::components::tooltip::NativeTooltipExt;
use crate::ui::theme::OxydeTheme;
use gpui::prelude::*;
use gpui::*;
pub(crate) fn toolbar_button_with_tooltip(
    id: String,
    tooltip: impl Fn(&mut Window, &mut App) -> AnyView + 'static,
    icon: Option<&'static str>,
    text: Option<String>,
    theme: OxydeTheme,
    enabled: bool,
    action: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    button_style(div().id(id), theme, ButtonVariant::Ghost, enabled)
        .size(rems(2.0))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .native_tooltip(tooltip)
        .when(enabled, |d| d.on_mouse_down(MouseButton::Left, action))
        .when_some(icon, |d, path| {
            d.child(svg().path(path).size(rems(1.125)).text_color(if enabled {
                theme.text
            } else {
                theme.icon_disabled
            }))
        })
        .when_some(text, |d, text| d.child(text))
}
