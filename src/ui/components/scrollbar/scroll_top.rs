use crate::ui::{elements::motion, scale::logical, theme::OxydeTheme};
use gpui::{prelude::*, *};

/// Shared proximity zone for panes and Viewer; owners keep their hover state.
pub(crate) fn hover_zone(
    zone: Div,
    id: String,
    on_hover: impl Fn(&bool, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    zone.id(id).group("scroll-top-zone").on_hover(on_hover)
}

/// Shared affordance following the vertical thumb; the owner performs scrolling.
pub(crate) fn button(
    theme: OxydeTheme,
    id: String,
    hovered: bool,
    anchor: &std::cell::Cell<f32>,
    offset: f32,
    viewport: f32,
    content: f32,
    on_press: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    if content <= viewport {
        return div().absolute().size_0().into_any_element();
    }
    let shown = hovered && offset > crate::ui::style::row_height() * 2.0;
    let thumb = (viewport * (viewport / content.max(1.0)).min(1.0))
        .max(logical(40.0)).min(viewport);
    let thumb_top = (offset / (content - viewport).max(1.0)).clamp(0.0, 1.0) * (viewport - thumb);
    let top = (thumb_top + thumb / 2.0 - logical(16.0))
        .clamp(logical(4.0), (viewport - logical(36.0)).max(logical(4.0)));
    let top = if shown { anchor.set(top); top } else { anchor.get() };
    let rise = logical(4.0);
    let presence_id = SharedString::from(format!("scroll-to-top-presence-{id}"));
    div()
        .id(format!("scroll-to-top-{id}"))
        .absolute()
        .right(rems(1.0))
        .size(rems(2.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(rems(0.5))
        .bg(theme.accent)
        .shadow(crate::ui::style::rem_shadow(false))
        .when(shown, |button| {
            button.hover(|style| style.opacity(0.9))
                .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                    cx.stop_propagation();
                    on_press(event, window, cx);
                })
        })
        .child(svg().path("icons/arrow-up.svg").size(rems(1.0)).text_color(theme.text_on_accent))
        .with_spring(presence_id, motion::presence(shown).from(0.0), move |button, progress| {
            button.top(px(top + (1.0 - progress) * rise)).opacity(progress)
        })
        .into_any_element()
}
