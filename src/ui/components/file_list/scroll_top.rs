use crate::ui::{app::OxydeApp, scale::logical, theme::OxydeTheme};
use gpui::{prelude::*, *};

pub(super) fn button(
    theme: OxydeTheme,
    handle: Entity<OxydeApp>,
    pane_id: String,
    offset: f32,
    viewport: f32,
    content: f32,
) -> AnyElement {
    if offset <= crate::ui::style::row_height() * 2.0 || content <= viewport {
        return div().absolute().size_0().into_any_element();
    }
    let thumb = (viewport * (viewport / content.max(1.0)).min(1.0))
        .max(logical(40.0))
        .min(viewport);
    let thumb_top = (offset / (content - viewport).max(1.0)).clamp(0.0, 1.0) * (viewport - thumb);
    let top = (thumb_top + thumb / 2.0 - logical(16.0))
        .clamp(logical(4.0), (viewport - logical(36.0)).max(logical(4.0)));
    div()
        .id(format!("scroll-to-top-{pane_id}"))
        .absolute()
        .top(px(top))
        .right(rems(1.0))
        .size(rems(2.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(rems(0.5))
        .bg(theme.accent)
        .shadow(crate::ui::style::rem_shadow(false))
        .opacity(0.0)
        .group_hover("file-list-scroll-zone", |style| style.opacity(1.0))
        .hover(|style| style.opacity(0.9))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            cx.stop_propagation();
            handle.update(cx, |this, cx| {
                if let Some(pane) = this.state.find_pane_mut(&pane_id) {
                    pane.active_tab_mut()
                        .list_scroll
                        .0
                        .borrow()
                        .base_handle
                        .set_offset(point(px(0.0), px(0.0)));
                }
                cx.notify();
            });
        })
        .child(
            svg()
                .path("icons/arrow-up.svg")
                .size(rems(1.0))
                .text_color(theme.text_on_accent),
        )
        .into_any_element()
}
