//! The footer's menus, as the crumbs offer theirs (`GenericDropdown`, owned menu
//! windows where available): the application's type filters, and each of its choices
//! that offers a list of answers. One is open at a time.
use super::FileChooserWindow;
use crate::ui::components::dropdown::{DropdownItem, GenericDropdown, IconName};
use crate::ui::scale::logical;
use crate::ui::theme::OxydeTheme;
use gpui::prelude::*;
use gpui::*;
use std::rc::Rc;

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
            max_height: logical(360.0),
            header: None,
            on_close: Rc::new(move |_, cx| {
                let _ = close.update(cx, |this, cx| this.close_menu(cx));
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

    /// The button showing `label`, the answer given, which opens `menu` above it.
    pub(super) fn render_menu(&self, menu: FooterMenu, label: String, theme: OxydeTheme, cx: &mut Context<Self>) -> AnyElement {
        let handle = cx.entity();
        let open = self.menu == Some(menu);
        let trigger = crate::ui::components::tool_window::action_button(menu.id(), SharedString::default(), false, cx)
            .max_w(rems(18.0))
            .child(div().min_w_0().truncate().child(label))
            .child(svg().path("icons/chevron-down.svg").size(rems(0.75)).flex_none().text_color(theme.text))
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                this.menu = if this.menu == Some(menu) { None } else { Some(menu) };
                this.menu_scroll = UniformListScrollHandle::new();
                cx.notify();
            }));
        div()
            .relative()
            .min_w_0()
            .flex_none()
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
