//! The application's type filters, offered in a menu at the start of the footer, as
//! the crumbs offer theirs (`GenericDropdown`, owned menu windows where available).
//! The filter shown decides which files the list offers and goes back with the choice.
use super::FileChooserWindow;
use crate::ui::components::dropdown::{DropdownItem, GenericDropdown, IconName};
use crate::ui::scale::logical;
use crate::ui::theme::OxydeTheme;
use gpui::prelude::*;
use gpui::*;
use std::rc::Rc;

impl FileChooserWindow {
    fn set_type_filter(&mut self, position: usize, cx: &mut Context<Self>) {
        self.type_menu = false;
        if self.type_filter != Some(position) && position < self.filters.len() {
            self.type_filter = Some(position);
            self.admission.types = self.filters.get(position).cloned();
            let keep = self.kept();
            self.arrange(keep, cx);
        }
        cx.notify();
    }

    fn type_menu(&self, handle: Entity<Self>) -> GenericDropdown<Self> {
        let items = self.filters.iter().enumerate().map(|(position, filter)| {
            let choose = handle.downgrade();
            DropdownItem {
                label: filter.name.clone(),
                badge: None,
                icon: (self.type_filter == Some(position)).then_some(IconName::Check),
                stay_open: false,
                has_submenu: false,
                is_dimmed: false,
                on_click: Rc::new(move |_, cx| {
                    let _ = choose.update(cx, |this, cx| this.set_type_filter(position, cx));
                }),
                on_hover: None,
            }
        }).collect();
        let close = handle.downgrade();
        GenericDropdown {
            id: "file-chooser-type-menu".into(),
            h_app: handle,
            items,
            reserve_icon_space: true,
            is_context_menu: false,
            scroll_handle: self.type_menu_scroll.clone(),
            width: None,
            max_height: logical(360.0),
            header: None,
            on_close: Rc::new(move |_, cx| {
                let _ = close.update(cx, |this, cx| {
                    this.type_menu = false;
                    cx.notify();
                });
            }),
            on_outside: None,
            // The menu scrolls with the wheel; its thumb is not dragged here.
            on_drag: Rc::new(|_, _, _, _, _| {}),
            is_dragging: false,
            on_item_hover: None,
            on_width_calculated: None,
            on_hover_change: None,
        }
    }

    /// The filter shown and the menu of the others; nothing when the application gives
    /// none, or when it asks for folders, which filters leave alone.
    pub(super) fn render_type_filter(&self, theme: OxydeTheme, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.directory { return None; }
        let label = self.filters.get(self.type_filter?)?.name.clone();
        let handle = cx.entity();
        let trigger = crate::ui::components::tool_window::action_button("file-chooser-types", SharedString::default(), false, cx)
            .max_w(rems(18.0))
            .child(div().min_w_0().truncate().child(label))
            .child(svg().path("icons/chevron-down.svg").size(rems(0.75)).flex_none().text_color(theme.text))
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                this.type_menu = !this.type_menu;
                cx.notify();
            }));
        Some(div()
            .relative()
            .min_w_0()
            .flex_none()
            .child(crate::ui::components::popup::anchored_surface("file-chooser-type-menu", handle.clone(),
                |this| this.type_menu,
                |this, _, cx| if this.type_menu { this.type_menu(cx.entity()).into_any_element() } else { div().into_any_element() },
                |this, _, cx| { this.type_menu = false; cx.notify(); }))
            .child(trigger)
            // Where owned menu windows are not available, the menu draws in this window.
            .when(self.type_menu && !crate::ui::components::popup::supported_source(&handle, cx), |filter| {
                filter.child(deferred(anchored().anchor(Anchor::BottomLeft).child(self.type_menu(handle.clone()))))
            })
            .into_any_element())
    }
}
