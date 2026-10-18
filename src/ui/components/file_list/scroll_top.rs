use crate::ui::{app::OxydeApp, elements::motion, scale::logical, theme::OxydeTheme};
use gpui::{prelude::*, *};

/// Tracks the pointer over the vertical scrollbar zone so the button can fade with it.
pub(super) fn hover_zone(zone: Div, handle: Entity<OxydeApp>, pane_id: &str) -> Stateful<Div> {
    let pane = pane_id.to_string();
    zone.id(SharedString::from(format!("file-list-scroll-zone-{pane_id}")))
        .on_hover(move |hovered, _, cx| {
            handle.update(cx, |this, cx| {
                let current = this.scroll_zone_hover.as_deref() == Some(pane.as_str());
                if *hovered == current {
                    return;
                }
                this.scroll_zone_hover = hovered.then(|| pane.clone());
                cx.notify();
            });
        })
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
    if content <= viewport {
        return div().absolute().size_0().into_any_element();
    }
    let shown = hovered && offset > crate::ui::style::row_height() * 2.0;
    let thumb = (viewport * (viewport / content.max(1.0)).min(1.0))
        .max(logical(40.0))
        .min(viewport);
    let thumb_top = (offset / (content - viewport).max(1.0)).clamp(0.0, 1.0) * (viewport - thumb);
    let top = (thumb_top + thumb / 2.0 - logical(16.0))
        .clamp(logical(4.0), (viewport - logical(36.0)).max(logical(4.0)));
    // Fade out where it was last shown, not where the scrollbar jumps.
    let top = if shown {
        anchor.set(top);
        top
    } else {
        anchor.get()
    };
    let rise = logical(4.0);
    let presence_id = SharedString::from(format!("scroll-to-top-presence-{pane_id}"));
    div()
        .id(format!("scroll-to-top-{pane_id}"))
        .absolute()
        .right(rems(1.0))
        .size(rems(2.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(rems(0.5))
        .bg(theme.accent)
        .shadow(crate::ui::style::rem_shadow(false))
        // Hidden buttons stay mounted so they can fade out, but never take clicks.
        .when(shown, |button| {
            button.hover(|style| style.opacity(0.9)).on_mouse_down(MouseButton::Left, move |_, _, cx| {
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
        })
        .child(
            svg()
                .path("icons/arrow-up.svg")
                .size(rems(1.0))
                .text_color(theme.text_on_accent),
        )
        .with_spring(
            presence_id,
            motion::presence(shown).from(0.0),
            move |button, progress| button.top(px(top + (1.0 - progress) * rise)).opacity(progress),
        )
        .into_any_element()
}
