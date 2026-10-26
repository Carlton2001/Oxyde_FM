use crate::ui::app::{ContextMenuContent, ContextMenuInstance, DragTarget, OxydeApp};
use crate::ui::app::context_menus::{context_menu_contains_point, context_menu_height, context_menu_visible_top, native_menu_max_height};
use crate::ui::components::dropdown::GenericDropdown;
use crate::ui::scale::{logical, rem_pixels};
use gpui::*;

impl OxydeApp {
    // Preserve the original clone semantics and bodies during structural extraction.
    #[allow(clippy::ptr_arg)]
    pub(super) fn render_native_menu(
        &self,
        path: &Vec<String>,
        is_background: &bool,
        items: &Vec<oxyde_lib::models::WinMenuItem>,
        instance: &ContextMenuInstance,
        m_idx: usize,
        h: Entity<Self>,
        scroll_handle: UniformListScrollHandle,
        is_v_dragging: bool,
        window_handle: AnyWindowHandle,
        win_h: f32,
    ) -> GenericDropdown {
        let path_cap = path.clone();
        let is_bg_cap = *is_background;
        let h_app = h.clone();
        let items_cap = items.clone();

        let dropdown_items = items
            .iter()
            .map(|it| {
                let h = h_app.clone();
                let p = path_cap.clone();
                let id = it.id;
                let has_sub = it.has_submenu;

                crate::ui::components::dropdown::DropdownItem {
                    label: it.label.clone(),
                    icon: if m_idx == 1 {
                        crate::ui::components::dropdown::native_menu_icon(
                            it.verb.as_deref(),
                            &it.label,
                        )
                    } else {
                        None
                    },
                    badge: None,
                    stay_open: has_sub,
                    has_submenu: has_sub,
                    // Windows returns -1 for popup entries that own a submenu.
                    // That is not a disabled command; only dim actual leaf entries
                    // whose IDs are invalid.
                    is_dimmed: id < 0 && !has_sub,
                    on_click: std::rc::Rc::new({
                        move |_, app| {
                            let h_inner = h.clone();
                            let p_inner = p.clone();
                            let id_inner = id;

                            h_inner.update(app, |this, cx| {
                                if !has_sub && id_inner >= 0 {
                                    fn is_creation(
                                        items: &[oxyde_lib::models::WinMenuItem],
                                        id: i32,
                                        in_new: bool,
                                    ) -> bool
                                    {
                                        items.iter().any(|item| {
                                            let in_new = in_new
                                                || item.verb.as_deref() == Some("new");
                                            (item.id == id && in_new)
                                                || is_creation(
                                                    &item.children,
                                                    id,
                                                    in_new,
                                                )
                                        })
                                    }
                                    let creates_item = is_bg_cap
                                        && this.context_menus.iter().any(|menu| {
                                            if let ContextMenuContent::NativeItems {
                                                items,
                                                ..
                                            } = &menu.content
                                            {
                                                is_creation(items, id_inner, false)
                                            } else {
                                                false
                                            }
                                        });
                                    this.execute_native_context_menu_item(
                                        p_inner,
                                        id_inner,
                                        is_bg_cap,
                                        creates_item,
                                        cx,
                                    );
                                }
                                cx.notify();
                            });
                        }
                    }),
                    on_hover: None,
                }
            })
            .collect();

        GenericDropdown {
            id: ("context-menu", m_idx).into(),
            h_app: h.clone(),
            items: dropdown_items,
            reserve_icon_space: m_idx == 1,
            is_context_menu: true,
            bottom_padding: instance.bottom_padding,
            scroll_handle,
            width: None,
            max_height: native_menu_max_height(px(win_h)),
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
                        let h_inner = h_app.clone();
                        let path_inner = path_cap.clone();
                        let sub_items = it.children.clone();
                        let has_sub = it.has_submenu;
                        let window_handle_inner = window_handle;

                        cx.defer(move |cx| {
                            let h_inner_spawn = h_inner.clone();
                            h_inner.update(cx, |this, cx| {
                                if has_sub && !sub_items.is_empty() {
                                    let path_cap = path_inner.clone();
                                    let sub_items_cap = sub_items.clone();
                                    let pos_inner = item_pos;
                                    let parent_left = this.context_menus.get(m_idx)
                                        .map(|m| m.position.x)
                                        .unwrap_or_default();
                                    let parent_right = parent_left + this.context_menus.get(m_idx)
                                        .map(|m| m.width)
                                        .unwrap_or_default();
                                    let parent_pos_y = this.context_menus.get(m_idx)
                                        .map(|m| m.position.y)
                                        .unwrap_or_default();
                                    let parent_menu_height = this.context_menus.get(m_idx)
                                        .map(|menu| context_menu_height(menu, px(win_h)))
                                        .unwrap_or_default();
                                    let ix_cap = ix;

                                    this.submenu_close_timer = None;
                                    this.submenu_timer = Some(cx.spawn(move |_, cx: &mut AsyncApp| {
                                        let mut cx_val = cx.clone();
                                        async move {
                                            cx_val.background_executor().timer(std::time::Duration::ZERO).await;
                                            let _ = window_handle_inner.update(&mut cx_val, |_, _window, cx| {
                                                let viewport_w = crate::ui::components::client_decorations::content_bounds(_window).right();
                                                let parent_visible_y = context_menu_visible_top(
                                                    parent_pos_y,
                                                    parent_menu_height,
                                                    crate::ui::components::client_decorations::content_bounds(_window).bottom(),
                                                );
                                                let open_left = parent_right + rem_pixels(rems(13.75)) > viewport_w;
                                                let app_handle = h_inner_spawn.clone();
                                                let path_cap = path_cap.clone();
                                                let sub_items_cap = sub_items_cap.clone();

                                                cx.defer(move |cx| {
                                                    app_handle.update(cx, |this, cx| {
                                                        let is_already_open = this.context_menus.get(m_idx + 1)
                                                            .map(|m| matches!(&m.content, ContextMenuContent::NativeItems { items, .. } if items.first().map(|it| it.id) == sub_items_cap.first().map(|it| it.id)))
                                                            .unwrap_or(false);

                                                        if !is_already_open {
                                                            this.context_menus.truncate(m_idx + 1);
                                                            let mut new_pos = pos_inner;
                                                            new_pos.x = if open_left {
                                                                parent_left + rem_pixels(rems(0.125)) // TopRight: right edge at parent left
                                                            } else {
                                                                parent_right - rem_pixels(rems(0.125)) // TopLeft: left edge at parent right
                                                            };
                                                            new_pos.y = parent_visible_y + px(ix_cap as f32 * logical(26.0));

                                                            this.context_menus.push(ContextMenuInstance {
            native_bounds: None,
                                                                position: new_pos,
                                                                open_left,
                                                                hovered_item: None,
                                                                native_request_id: None, open_up: false,
                                                                content: ContextMenuContent::NativeItems {
                                                                    path: path_cap,
                                                                    is_background: is_bg_cap,
                                                                    items: sub_items_cap
                                                                },
                                                                scroll_handle: UniformListScrollHandle::new(),
                        width: px(0.0),
                        bottom_padding: 0.0,
                    });
                                                            cx.notify();
                                                        }
                                                    });
                                                });
                                            });
                                        }
                                    }));
                                } else {
                                    this.submenu_timer = None;
                                    this.submenu_close_timer = None;
                                    if this.context_menus.len() > m_idx + 1 {
                                        this.context_menus.truncate(m_idx + 1);
                                        cx.notify();
                                    }
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
                                                    if this.context_menus.len()
                                                        > m_idx + 1
                                                    {
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
