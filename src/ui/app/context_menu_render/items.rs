use crate::ui::app::{DragTarget, OxydeApp};
use crate::ui::app::context_menus::{context_menu_contains_point, context_menu_height, context_menu_visible_top};
use crate::ui::components::dropdown::{DropdownItem, GenericDropdown};
use crate::ui::scale::rem_pixels;
use gpui::*;

impl OxydeApp {
    pub(super) fn render_items_menu(
        &self,
        items: &[DropdownItem],
        m_idx: usize,
        h: Entity<Self>,
        scroll_handle: UniformListScrollHandle,
        is_v_dragging: bool,
        window_handle: AnyWindowHandle,
        win_h: f32,
    ) -> GenericDropdown {
        let dropdown_items = items
            .iter()
            .map(|it| {
                let on_click = it.on_click.clone();
                crate::ui::components::dropdown::DropdownItem {
                    label: it.label.clone(),
                    icon: it.icon.clone(),
                    badge: it.badge.clone(),
                    stay_open: it.stay_open,
                    has_submenu: it.has_submenu,
                    is_dimmed: it.is_dimmed,
                    is_faded: false,
                    on_click: std::rc::Rc::new(move |window, app| {
                        (on_click)(window, app);
                    }),
                    on_hover: it.on_hover.clone(),
                }
            })
            .collect();

        let items_cap = items.to_vec();
        GenericDropdown {
            id: ("context-menu", m_idx).into(),
            h_app: h.clone(),
            items: dropdown_items,
            reserve_icon_space: true,
            is_context_menu: true,
            scroll_handle,
            width: None,
            min_width: None,
            // Only the window bounds the menu, as the native menus.
            max_height: f32::INFINITY,
            on_close: {
                let h = h.clone();
                std::rc::Rc::new(move |_, cx| {
                    h.update(cx, |this, cx| {
                        if this.context_menus.get(m_idx.wrapping_sub(1)).is_some_and(|menu| matches!(menu.content, crate::ui::app::ContextMenuContent::ColumnFilter(_))) {
                            this.context_menus.truncate(m_idx);
                        } else {
                            this.context_menus.clear();
                        }
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
                        let inside_sibling = this.context_menus.iter().enumerate().any(
                            |(index, menu)| {
                                index != m_idx
                                    && context_menu_contains_point(
                                        menu,
                                        position,
                                        px(win_h),
                                    )
                            },
                        );
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
            is_dragging: is_v_dragging,
            header: None,
            footer: None,
            on_item_hover: Some(std::rc::Rc::new({
                let h_app = h.clone();
                let items_val = items_cap.clone();
                move |ix, item_pos, cx| {
                    let should_handle = h_app.update(cx, |this, cx| {
                        let Some(menu) = this.context_menus.get_mut(m_idx) else {
                            return false;
                        };
                        if menu.hovered_item == Some(ix) {
                            return false;
                        }
                        menu.hovered_item = Some(ix);
                        cx.notify();
                        true
                    });
                    if !should_handle {
                        return;
                    }
                    if let Some(it) = items_val.get(ix) {
                        let has_submenu = it.on_hover.is_some();
                        let h_inner = h_app.clone();
                        let items_inner = items_val.clone();
                        let ix_inner = ix;
                        let pos_inner = item_pos;
                        let window_handle_inner = window_handle;

                        if has_submenu {
                            cx.defer(move |cx| {
                                let _h_inner_spawn = h_inner.clone();
                                h_inner.update(cx, |this, cx| {
                                    let parent_left = this
                                        .context_menus
                                        .get(m_idx)
                                        .map(|m| m.native_bounds.map(|bounds| bounds.left()).unwrap_or(m.position.x))
                                        .unwrap_or_default();
                                    let parent_right = parent_left
                                        + this
                                            .context_menus
                                            .get(m_idx)
                                            .map(|m| m.native_bounds.map(|bounds| bounds.size.width).unwrap_or(m.width))
                                            .unwrap_or_default();
                                    // Compute Y from parent menu position + item index (reliable, no canvas-bounds dependency)
                                    let parent_pos_y = this
                                        .context_menus
                                        .get(m_idx)
                                        .map(|m| m.native_bounds.map(|bounds| bounds.top()).unwrap_or(m.position.y))
                                        .unwrap_or_default();
                                    let parent_menu_height = this
                                        .context_menus
                                        .get(m_idx)
                                        .map(|menu| {
                                            context_menu_height(menu, px(win_h))
                                        })
                                        .unwrap_or_default();
                                    this.submenu_close_timer = None;
                                    this.submenu_timer =
                                        Some(cx.spawn(move |_, cx: &mut AsyncApp| {
                                            let mut cx_val = cx.clone();
                                            async move {
                                                cx_val
                                                    .background_executor()
                                                    .timer(std::time::Duration::ZERO)
                                                    .await;
                                                let _ = window_handle_inner.update(
                                                    &mut cx_val,
                                                    |_, _window, app| {
                                                        let viewport_w = crate::ui::components::client_decorations::content_bounds(_window).right();
                                                        let parent_visible_y =
                                                            context_menu_visible_top(
                                                                parent_pos_y,
                                                                parent_menu_height,
                                                                crate::ui::components::client_decorations::content_bounds(_window).bottom(),
                                                            );
                                                        let open_left = parent_right
                                                            + rem_pixels(rems(13.75))
                                                            > viewport_w;
                                                        if let Some(it) =
                                                            items_inner.get(ix_inner)
                                                        {
                                                            if let Some(on_h) =
                                                                it.on_hover.clone()
                                                            {
                                                                let mut sub_pos =
                                                                    pos_inner;
                                                                sub_pos.x = if open_left
                                                                {
                                                                    parent_left
                                                                        + rem_pixels(rems(0.125))
                                                                // TopRight anchor: right edge at parent left
                                                                } else {
                                                                    parent_right
                                                                        - rem_pixels(rems(0.125))
                                                                    // TopLeft anchor: left edge at parent right
                                                                };
                                                                // Y: parent menu top + border(1) + padding(4) + ix * item_h
                                                                // minus submenu border(1) + padding(4) → nets to ix * item_h
                                                                sub_pos.y =
                                                                    parent_visible_y
                                                                        + px(items_inner.iter().take(ix_inner).filter(|item| !matches!(item.icon, Some(crate::ui::components::dropdown::IconName::Separator))).count()
                                                                            as f32
                                                                            * crate::ui::style::menu_item_height() + items_inner.iter().take(ix_inner).filter(|item| matches!(item.icon, Some(crate::ui::components::dropdown::IconName::Separator))).count() as f32 * crate::ui::style::menu_separator_height());
                                                                (on_h)(
                                                                    sub_pos, open_left,
                                                                    app,
                                                                );
                                                            }
                                                        }
                                                    },
                                                );
                                            }
                                        }));
                                });
                            });
                        } else {
                            // Item has no submenu → close any open children immediately
                            cx.defer(move |cx| {
                                h_inner.update(cx, |this, cx| {
                                    this.submenu_timer = None;
                                    this.submenu_close_timer = None;
                                    // Hovering a sibling closes even the submenus that survive the pointer leaving.
                                    if this.context_menus.len() > m_idx + 1 {
                                        this.context_menus.truncate(m_idx + 1);
                                        cx.notify();
                                    }
                                });
                            });
                        }
                    } else {
                        let h_close = h_app.clone();
                        cx.defer(move |cx| {
                            h_close.update(cx, |this, cx| {
                                this.submenu_timer = None;
                                this.submenu_close_timer = None;
                                if this.context_menus.len() > m_idx + 1 {
                                    this.context_menus.truncate(m_idx + 1);
                                    cx.notify();
                                }
                            });
                        });
                    }
                }
            })),
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
            on_hover_change: Some(std::rc::Rc::new({
                let h = h.clone();
                move |is_hovering: bool, cx: &mut App| {
                    if is_hovering {
                        h.update(cx, |this, _| {
                            this.submenu_close_timer = None;
                        });
                    } else {
                        h.update(cx, |this, _| {
                            if let Some(menu) = this.context_menus.get_mut(m_idx) {
                                menu.hovered_item = None;
                            }
                        });
                        let h_close = h.clone();
                        let wh = window_handle;
                        h.update(cx, |this, cx| {
                            this.submenu_close_timer =
                                Some(cx.spawn(move |_, cx: &mut AsyncApp| {
                                    let mut cx_val = cx.clone();
                                    async move {
                                        cx_val
                                            .background_executor()
                                            .timer(std::time::Duration::from_millis(
                                                250,
                                            ))
                                            .await;
                                        let _ = wh.update(&mut cx_val, |_, _, cx| {
                                            let h_inner = h_close.clone();
                                            cx.defer(move |cx| {
                                                h_inner.update(cx, |this, cx| {
                                                    if this.context_menus.len() > m_idx + 1 && !this.menu_stays_open(m_idx + 1) {
                                                        this.context_menus
                                                            .truncate(m_idx + 1);
                                                        cx.notify();
                                                    }
                                                });
                                            });
                                        });
                                    }
                                }));
                        });
                    }
                }
            })),
        }
    }
}
