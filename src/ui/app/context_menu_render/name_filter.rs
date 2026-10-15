use crate::ui::app::{ContextMenuInstance, DragTarget, OxydeApp};
use crate::ui::app::context_menus::context_menu_contains_point;
use crate::ui::components::dropdown::GenericDropdown;
use crate::ui::components::input::TextInput;
use crate::ui::scale::logical;
use gpui::prelude::*;
use gpui::*;

impl OxydeApp {
    pub(super) fn render_name_filter_menu(
        &self,
        input: &Entity<TextInput>,
        instance: &ContextMenuInstance,
        m_idx: usize,
        h: Entity<Self>,
        scroll_handle: UniformListScrollHandle,
        win_h: f32,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> GenericDropdown {
        if crate::ui::components::popup::is_menu(_window, cx) {
            let focus = input.read(cx).focus_handle.clone();
            if !focus.is_focused(_window) { _window.focus(&focus, cx); }
        }
        let input_width = (input.read(cx).placeholder_width(_window).as_f32() + logical(32.0)).max(logical(200.0));
        GenericDropdown {
        id: ("context-menu", m_idx).into(),
        h_app: h.clone(),
        items: vec![],
        reserve_icon_space: true,
        is_context_menu: true,
        bottom_padding: instance.bottom_padding,
        scroll_handle,
        width: Some(input_width + logical(8.0)),
        max_height: logical(480.0),
        on_close: {
            let h = h.clone();
            std::rc::Rc::new(move |_, cx| {
                h.update(cx, |this, cx| {
                    this.context_menus.clear();
                    this.submenu_timer = None;
                    this.submenu_close_timer = None;
                    cx.notify();
                });
            })
        },
        on_outside: Some({
            let h = h.clone();
            std::rc::Rc::new(move |position, _, cx| {
                h.update(cx, |this, cx| {
                    let inside_sibling =
                        this.context_menus.iter().enumerate().any(|(index, menu)| {
                            index != m_idx
                                && context_menu_contains_point(
                                    menu,
                                    position,
                                    px(win_h),
                                )
                        });
                    if !inside_sibling {
                        this.context_menus.clear();
                        this.submenu_timer = None;
                        this.submenu_close_timer = None;
                        cx.notify();
                    }
                });
            })
        }),
        on_drag: {
            std::rc::Rc::new(move |this, tid, vh, ch, _| {
                this.drag_target = Some(DragTarget::ContextMenuV {
                    menu_idx: m_idx,
                    viewport_h: vh,
                    content_h: ch,
                    track_id: tid,
                });
            })
        },
        is_dragging: false,
        header: Some(
            div()
                .w(px(input_width))
                .child(input.clone())
                .into_any_element(),
        ),
        on_item_hover: None,
        on_width_calculated: Some(std::rc::Rc::new({
            let h = h.clone();
            move |w, cx| {
                h.update(cx, |this, _| {
                    if let Some(menu) = this.context_menus.get_mut(m_idx) {
                        menu.width = w;
                    }
                });
            }
        })),
        on_hover_change: None,
    }
    }
}
