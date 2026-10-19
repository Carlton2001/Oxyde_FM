use crate::ui::{app::OxydeApp, theme::OxydeTheme};
use gpui::*;

/// Tracks the pointer over the vertical scrollbar zone so the button can fade with it.
pub(super) fn hover_zone(zone: Div, handle: Entity<OxydeApp>, pane_id: &str) -> Stateful<Div> {
    let pane = pane_id.to_string();
    crate::ui::components::scrollbar::scroll_top::hover_zone(
        zone, format!("file-list-scroll-zone-{pane_id}"), move |hovered, _, cx| {
            handle.update(cx, |this, cx| {
                let current = this.scroll_zone_hover.as_deref() == Some(pane.as_str());
                if *hovered == current {
                    return;
                }
                this.scroll_zone_hover = hovered.then(|| pane.clone());
                cx.notify();
            });
        },
    )
}

pub(super) fn button(
    theme: OxydeTheme,
    handle: Entity<OxydeApp>,
    pane_id: String,
    hovered: bool,
    anchor: &std::cell::Cell<f32>,
    offset: f32,
    viewport: f32,
    content: f32,
) -> AnyElement {
    crate::ui::components::scrollbar::scroll_top::button(
        theme, pane_id.clone(), hovered, anchor, offset, viewport, content,
        move |_, _, cx| {
            handle.update(cx, |this, cx| {
                if let Some(pane) = this.state.find_pane_mut(&pane_id) {
                    pane.active_tab_mut().list_scroll.0.borrow().base_handle
                        .set_offset(point(px(0.0), px(0.0)));
                }
                cx.notify();
            });
        },
    )
}
