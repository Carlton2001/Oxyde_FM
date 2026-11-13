//! The footer's menus, as the crumbs offer theirs (`GenericDropdown`, owned menu
//! windows where available): the application's type filters, and each of its choices
//! that offers a list of answers. One is open at a time.
use super::FileChooserWindow;
use crate::ui::components::dropdown::{DropdownItem, GenericDropdown, IconName};
use crate::ui::scale::logical;
use gpui::prelude::*;
use gpui::*;
use std::rc::Rc;

/// The width of the footer's menus.
pub(super) const MENU_WIDTH: Rems = Rems(16.0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FooterMenu {
    Types,
    /// A choice's answers, by its position.
    Choice(usize),
}

impl FooterMenu {
    fn id(self) -> SharedString {
        match self {
            Self::Types => "file-chooser-type-menu".into(),
            Self::Choice(position) => format!("file-chooser-choice-menu-{position}").into(),
        }
    }
}

impl FileChooserWindow {
    /// The menu's labels, and the one answered.
    fn menu_entries(&self, menu: FooterMenu) -> (Vec<String>, Option<usize>) {
        match menu {
            FooterMenu::Types => (self.filters.iter().map(|filter| filter.name.clone()).collect(), self.type_filter),
            FooterMenu::Choice(position) => self.choices.get(position).map(|choice| choice.entries()).unwrap_or_default(),
        }
    }

    fn choose_in_menu(&mut self, menu: FooterMenu, position: usize, cx: &mut Context<Self>) {
        self.menu = None;
        match menu {
            FooterMenu::Types => self.set_type_filter(position, cx),
            FooterMenu::Choice(choice) => self.answer_choice(choice, position),
        }
        cx.notify();
    }

    fn toggle_menu(&mut self, menu: FooterMenu, cx: &mut Context<Self>) {
        self.menu = if self.menu == Some(menu) { None } else { Some(menu) };
        self.menu_scroll = UniformListScrollHandle::new();
        cx.notify();
    }

    fn close_menu(&mut self, cx: &mut Context<Self>) {
        self.menu = None;
        cx.notify();
    }

    fn dropdown(&self, menu: FooterMenu, handle: Entity<Self>) -> GenericDropdown<Self> {
        let (labels, answered) = self.menu_entries(menu);
        let items = labels.into_iter().enumerate().map(|(position, label)| {
            let choose = handle.downgrade();
            DropdownItem {
                label,
                badge: None,
                icon: (answered == Some(position)).then_some(IconName::Check),
                stay_open: false,
                has_submenu: false,
                is_dimmed: false,
                is_faded: false,
                on_click: Rc::new(move |_, cx| {
                    let _ = choose.update(cx, |this, cx| this.choose_in_menu(menu, position, cx));
                }),
                on_hover: None,
            }
        }).collect();
        let close = handle.downgrade();
        GenericDropdown {
            id: menu.id().into(),
            h_app: handle,
            items,
            reserve_icon_space: true,
            is_context_menu: false,
            scroll_handle: self.menu_scroll.clone(),
            width: None,
            min_width: Some(crate::ui::components::select_menu::list_width(
                self.menu_trigger_widths.borrow().get(&menu.id()).copied().unwrap_or_default(),
            ).as_f32()),
            max_height: logical(360.0),
            header: None,
            on_close: Rc::new(move |_, cx| {
                let _ = close.update(cx, |this, cx| this.close_menu(cx));
            }),
            on_outside: None,
            on_drag: Rc::new({
                let scroll = self.menu_scroll.0.borrow().base_handle.clone();
                move |this, track, viewport, content, _| this.start_menu_drag(track, viewport, content, scroll.clone())
            }),
            is_dragging: self.dragging_menu(),
            on_item_hover: None,
            on_width_calculated: None,
            on_hover_change: None,
        }
    }

    /// The button showing `label`, the answer given, which opens `menu` above it.
    pub(super) fn render_menu(&self, menu: FooterMenu, label: String, focus: &FocusHandle, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let handle = cx.entity();
        let open = self.menu == Some(menu);
        let widths = self.menu_trigger_widths.clone();
        let key = menu.id();
        let focused = crate::ui::elements::focus_ring::keyboard_focused(focus, window);
        let trigger = crate::ui::components::select_menu::trigger(menu.id(), label, focused, true, cx)
            .track_focus(focus)
            .w_full()
            .child(canvas(|bounds, _, _| bounds, move |bounds, _, _, _| {
                widths.borrow_mut().insert(key.clone(), bounds.size.width);
            }).absolute().inset_0().size_full())
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| this.toggle_menu(menu, cx)))
            // Space, Enter or Down while it has the keyboard opens it.
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                if matches!(event.keystroke.key.as_str(), "space" | "enter" | "down") && !event.keystroke.modifiers.modified() {
                    this.toggle_menu(menu, cx);
                    cx.stop_propagation();
                }
            }));
        // One width for every menu, which keeps the form's column straight.
        div()
            .relative()
            .w(MENU_WIDTH)
            .min_w_0()
            .child(crate::ui::components::popup::anchored_surface(menu.id(), handle.clone(),
                move |this| this.menu == Some(menu),
                move |this, _, cx| if this.menu == Some(menu) { this.dropdown(menu, cx.entity()).into_any_element() } else { div().into_any_element() },
                |this, _, cx| this.close_menu(cx)))
            .child(trigger)
            // Where owned menu windows are not available, the menu draws in this window.
            .when(open && !crate::ui::components::popup::supported_source(&handle, cx), |control| {
                control.child(deferred(anchored().anchor(Anchor::BottomLeft).child(self.dropdown(menu, handle.clone()))))
            })
            .into_any_element()
    }
}
