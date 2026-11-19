//! The grid's card sizes as one line of squares, each as large as the cards it chooses,
//! the current one selected. A click chooses and closes, as a menu entry does.
use crate::ui::app::context_menus::context_menu_contains_point;
use crate::ui::app::OxydeApp;
use crate::ui::components::dropdown::GenericDropdown;
use crate::ui::components::popup::keyboard;
use crate::ui::components::tooltip::{NativeTooltipExt, Tooltip};
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::scale::{logical, rem_pixels};
use crate::ui::style::{GridSize, MENU_ITEM_HEIGHT};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;
use std::rc::Rc;

/// Space between two squares.
const GAP: Rems = rems(0.125);

impl OxydeApp {
    pub(super) fn render_grid_size_menu(
        &self,
        pane_id: &str,
        m_idx: usize,
        h: Entity<Self>,
        scroll_handle: UniformListScrollHandle,
        win_h: f32,
        cx: &mut Context<Self>,
    ) -> GenericDropdown {
        let theme = *cx.theme();
        let current = self.grid_size();
        let mut row = div().flex().gap(GAP);
        for (index, (size, icon, label)) in [
            (GridSize::Small, rems(0.75), Message::GridSizeSmall),
            (GridSize::Medium, rems(1.0), Message::GridSizeMedium),
            (GridSize::Large, rems(1.25), Message::GridSizeLarge),
        ].into_iter().enumerate() {
            let chosen = size == current;
            let (handle, pane_id) = (h.clone(), pane_id.to_string());
            let action: keyboard::Action = Rc::new(move |_, cx| {
                handle.update(cx, |this, cx| this.choose_grid_size(&pane_id, size, cx));
            });
            let click = action.clone();
            let button = div()
                .id(("grid-size", index))
                .size(MENU_ITEM_HEIGHT)
                .flex()
                .items_center()
                .justify_center()
                .rounded(rems(0.3125))
                .when(chosen, |button| button.bg(theme.selected))
                .when(!chosen, |button| button.hover(|style| style.bg(theme.menu_hover)))
                .child(svg().path("icons/square.svg").size(icon).flex_none().text_color(if chosen { theme.accent } else { theme.text }))
                .fixed_tooltip(Tooltip::text(cx.i18n().text(label)))
                .on_click(move |_, window, cx| click(window, cx));
            row = row.child(keyboard::item(button, action, None, None).horizontal());
        }
        // Three squares and their gaps, inside the menu's padding and border.
        let width = 3.0 * rem_pixels(MENU_ITEM_HEIGHT).as_f32() + 2.0 * rem_pixels(GAP).as_f32() + logical(10.0);

        GenericDropdown {
            id: ("context-menu", m_idx).into(),
            h_app: h.clone(),
            items: Vec::new(),
            reserve_icon_space: false,
            is_context_menu: true,
            scroll_handle,
            width: None,
            min_width: Some(width),
            max_height: f32::INFINITY,
            on_close: {
                let h = h.clone();
                Rc::new(move |_, cx| h.update(cx, |this, cx| {
                    this.context_menus.clear();
                    cx.notify();
                }))
            },
            on_outside: Some({
                let h = h.clone();
                Rc::new(move |position, _, cx| h.update(cx, |this, cx| {
                    let inside_sibling = this.context_menus.iter().enumerate()
                        .any(|(index, menu)| index != m_idx && context_menu_contains_point(menu, position, px(win_h)));
                    if !inside_sibling {
                        this.context_menus.clear();
                        cx.notify();
                    }
                }))
            }),
            on_drag: Rc::new(|_, _, _, _, _| {}),
            is_dragging: false,
            header: Some(row.into_any_element()),
            footer: None,
            on_item_hover: None,
            on_width_calculated: Some(Rc::new(move |width, cx| h.update(cx, |this, _| {
                if let Some(menu) = this.context_menus.get_mut(m_idx) {
                    menu.width = width;
                }
            }))),
            on_hover_change: None,
        }
    }
}
