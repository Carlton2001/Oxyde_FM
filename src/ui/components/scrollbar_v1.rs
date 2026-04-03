use gpui::*;
use gpui::prelude::*;
use gpui_component::PixelsExt;
use crate::ui::app::OxydeApp;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum ScrollbarAxis {
    Vertical,
    Horizontal,
}

const ZED_THUMB_MIN_SIZE: f32 = 40.0;
pub const ZED_TRACK_WIDTH: f32 = 12.0;
const ZED_THUMB_WIDTH_NORMAL: f32 = 5.0;
const ZED_THUMB_WIDTH_HOVER: f32 = 10.0;

/// Composant Scrollbar Visuel Réutilisable "Zed-like"
pub fn render_scrollbar(
    handle: Entity<OxydeApp>,
    axis: ScrollbarAxis,
    viewport_size: f32,
    content_size: f32,
    current_scroll: f32,
    is_dragging: bool,
    id_suffix: String,
    on_drag: impl Fn(&mut OxydeApp, f32, f32, f32, &mut Context<OxydeApp>) + 'static,
) -> AnyElement {
    let is_v = axis == ScrollbarAxis::Vertical;
    
    // Proportions
    let ratio = (viewport_size / content_size.max(1.0)).min(1.0);
    
    // Don't render scrollbar if content fits
    if ratio >= 1.0 {
        return div().into_any_element();
    }
    
    let thumb_size = (viewport_size * ratio).max(ZED_THUMB_MIN_SIZE);
    let max_offset = (content_size - viewport_size).max(1.0);
    // current_scroll is usually negative in GPUI for list offsets
    let thumb_pos = (current_scroll.abs() / max_offset) * (viewport_size - thumb_size);

    let mut thumb = div()
        .absolute()
        .bg(rgba(0x55555544)) // Plus subtil par défaut
        .rounded_md();

    if is_v {
        thumb = thumb
            .right(px(3.0))
            .top(px(thumb_pos))
            .h(px(thumb_size))
            .w(px(ZED_THUMB_WIDTH_NORMAL))
            .group_hover("scroll-track", |s| s.w(px(ZED_THUMB_WIDTH_HOVER)).right(px(1.0)).bg(rgba(0xaaaaaaff)));
        if is_dragging {
            thumb = thumb.w(px(ZED_THUMB_WIDTH_HOVER)).right(px(1.0)).bg(rgba(0xffffffff));
        }
    } else {
        thumb = thumb
            .bottom(px(3.0))
            .left(px(thumb_pos))
            .w(px(thumb_size))
            .h(px(ZED_THUMB_WIDTH_NORMAL))
            .group_hover("scroll-track", |s| s.h(px(ZED_THUMB_WIDTH_HOVER)).bottom(px(1.0)).bg(rgba(0xaaaaaaff)));
        if is_dragging {
            thumb = thumb.h(px(ZED_THUMB_WIDTH_HOVER)).bottom(px(1.0)).bg(rgba(0xffffffff));
        }
    }

        let is_v = axis == ScrollbarAxis::Vertical;
        let track_id = format!("scrollbar-track-{}-{}", if is_v { "v" } else { "h" }, id_suffix);
        let track_id_cap = track_id.clone();
        let handle_cap = handle.clone();

        let track_container = div()
            .id(SharedString::from(track_id))
            .group("scroll-track")
            .absolute()
            .when(is_v, |s: Stateful<Div>| s.top_0().right_0().bottom_0().w(px(ZED_TRACK_WIDTH)))
            .when(!is_v, |s: Stateful<Div>| s.left_0().bottom_0().right_0().h(px(ZED_TRACK_WIDTH)))
            .hover(|s| s.bg(rgba(0xffffff11))) 
            .child(
                canvas(
                    |bounds, _, _| bounds,
                    move |bounds, _, _, cx| {
                        let _ = handle_cap.update(cx, |this, _| {
                            this.scrollbar_origins.insert(track_id_cap.clone(), bounds.origin.map(|p| p.as_f32()));
                        });
                    }
                ).absolute().size_full()
            )
            .child(thumb)
        .on_mouse_down(MouseButton::Left, {
            let h = handle.clone();
            let track_id_for_drag = track_id.clone();
            move |e, _, cx| {
                let mouse_pos = if is_v { e.position.y.as_f32() } else { e.position.x.as_f32() };
                
                let _ = h.update(cx, |this, _| {
                    if let Some(origin) = this.scrollbar_origins.get(&track_id_for_drag) {
                        let track_origin_coord = if is_v { origin.y } else { origin.x };
                        let mouse_in_track = mouse_pos - track_origin_coord;
                        
                        let max_offset = (content_size - viewport_size).max(1.0);
                        let ratio = (viewport_size / content_size.max(1.0)).min(1.0);
                        let t_size = (viewport_size * ratio).max(ZED_THUMB_MIN_SIZE);
                        let t_pos = (current_scroll.abs() / max_offset) * (viewport_size - t_size);
                        
                        // Si on clique sur le pouce, on garde le point de grab relatif. 
                        // Sinon (clic sur la piste), on centre le pouce sur la souris.
                        let grab_offset = if mouse_in_track >= t_pos && mouse_in_track <= t_pos + t_size {
                            mouse_in_track - t_pos
                        } else {
                            t_size / 2.0
                        };
                        
                        this.grab_offset = grab_offset;
                        on_drag(this, track_id_for_drag.clone(), viewport_size, content_size, cx);
                    }
                });
            }
        });

        track_container.into_any_element()
}
