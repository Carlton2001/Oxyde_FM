use gpui::*;
use crate::ui::app::{OxydeApp, DragTarget, ContextMenuContent};
use crate::ui::workspace::SidebarItem;
use crate::ui::components::scrollbar::{render_scrollbar, ScrollbarAxis};
use crate::ui::theme::{OxydeTheme, ThemeContext, PixelsExt};

const SIDEBAR_ROW_H: f32 = 28.0;

pub fn render_sidebar(view: &OxydeApp, handle: Entity<OxydeApp>, _cx: &mut Context<OxydeApp>) -> impl IntoElement {
    let sidebar_items = view.state.sidebar_items.clone();
    let _sidebar_scroll = view.sidebar_scroll.clone();
    let theme = *_cx.theme();
    
    // CALCULATE DYNAMIC CONTENT WIDTH
    let calculated_content_w = sidebar_items.iter().map(|item| {
        match item {
            SidebarItem::Header(h) => h.len() as f32 * 8.0 + 20.0,
            SidebarItem::Pinned { item } => item.name.len() as f32 * 7.0 + 60.0,
            SidebarItem::Drive { drive, .. } => (drive.label.len() + drive.path.len() + 3) as f32 * 7.0 + 60.0,
            SidebarItem::Directory { node, depth, .. } => (*depth as f32 * 12.0) + (node.name.len() as f32 * 7.0) + 60.0,
        }
    }).fold(view.sidebar_width, |a, b| a.max(b));

    if (calculated_content_w - view.sidebar_content_width).abs() > 1.0 {
        let h = handle.clone();
        _cx.defer(move |cx| {
            let _ = h.update(cx, |this, _| {
                this.sidebar_content_width = calculated_content_w;
            });
        });
    }

    let viewport_w = view.sidebar_width;
    let content_w = calculated_content_w.max(viewport_w);
    let viewport_h = view.list_height;
    let content_h = (sidebar_items.len() as f32 * SIDEBAR_ROW_H).max(1.0);

    let mut container = div()
        .w_full()
        .h_full()
        .bg(theme.sidebar)
        .border_r_1()
        .border_color(theme.border)
        .p_1()
        .overflow_hidden()
        .relative()
        .child(
            div()
                .size_full()
                .child(
                    div()
                        .absolute()
                        .left(view.sidebar_scroll_h.offset().x)
                        .w(px(content_w))
                        .h_full()
                        .child({
                            uniform_list(
                                "sidebar-list",
                                sidebar_items.len(),
                                {
                                    let items = sidebar_items.clone();
                                    let h = handle.clone();
                                    move |range, _, cx| {
                                        range.map(|i| {
                                            let theme = OxydeTheme::global(cx);
                                            let item = &items[i];
                                            match item {
                                                SidebarItem::Header(label) => {
                                                    div()
                                                        .h(px(SIDEBAR_ROW_H))
                                                        .px_3()
                                                        .flex()
                                                        .items_center()
                                                        .pb_1()
                                                        .text_color(theme.text_muted)
                                                        .child(label.clone())
                                                        .into_any_element()
                                                }
                                                SidebarItem::Pinned { item } => {
                                                    let path = item.path.clone();
                                                    let h_inner = h.clone();
                                                    div()
                                                        .h(px(SIDEBAR_ROW_H))
                                                        .px_3()
                                                        .rounded_md()
                                                        .flex()
                                                        .items_center()
                                                        .cursor_pointer()
                                                        .hover(|s| s.bg(theme.hover))
                                                        .child(
                                                            div()
                                                                .w_4()
                                                                .h_4()
                                                                .mr_1()
                                                        )
                                                        .child(
                                                            div()
                                                                .flex_1()
                                                                .flex()
                                                                .items_center()
                                                                .on_mouse_down(MouseButton::Left, {
                                                                    let p = path.clone();
                                                                    let h_inner_inner = h_inner.clone();
                                                                    move |e, _, cx| {
                                                                        if e.click_count != 2 {
                                                                            let _ = h_inner_inner.update(cx, |this, cx| {
                                                                                let p_id = this.state.active_pane_id.clone();
                                                                                this.navigate_to(&p_id, p.clone(), cx);
                                                                            });
                                                                        }
                                                                    }
                                                                })
                                                                .on_mouse_down(MouseButton::Middle, {
                                                                    let p = path.clone();
                                                                    let h_inner_inner = h_inner.clone();
                                                                    move |_, _, cx| {
                                                                        let _ = h_inner_inner.update(cx, |view, cx| {
                                                                            let p_id = view.state.active_pane_id.clone();
                                                                            view.add_tab(&p_id, p.clone(), false, cx);
                                                                        });
                                                                    }
                                                                })
                                                                .child(
                                                                    div()
                                                                        .w_4()
                                                                        .h_4()
                                                                        .mr_2()
                                                                        .flex_shrink_0()
                                                                        .child(svg().path("icons/folder.svg").text_color(theme.accent).size_full())
                                                                )
                                                                .child(
                                                                    div()
                                                                        .flex_1()
                                                                        .text_ellipsis()
                                                                        .overflow_hidden()
                                                                        .text_color(theme.text)
                                                                        .child(item.name.clone())
                                                                )
                                                        )
                                                        .on_mouse_down(MouseButton::Right, {
                                                            let h_right = h_inner.clone();
                                                            move |e, _, cx| {
                                                                let pos = e.position;
                                                                let _ = h_right.update(cx, |this, cx| {
                                                                    this.context_menu = Some((pos, ContextMenuContent::Items(super::oxyde_context_menu_items(h_right.clone()), None)));
                                                                    this.context_menu_scroll = UniformListScrollHandle::new();
                                                                    cx.notify();
                                                                });
                                                            }
                                                        })
                                                        .into_any_element()
                                                }
                                                SidebarItem::Drive { drive, is_expanded } => {
                                                    let path = drive.path.clone();
                                                    let h_inner = h.clone();
                                                    div()
                                                        .h(px(SIDEBAR_ROW_H))
                                                        .px_3()
                                                        .rounded_md()
                                                        .flex()
                                                        .items_center()
                                                        .cursor_pointer()
                                                        .hover(|s| s.bg(theme.hover))
                                                        .child(
                                                            div()
                                                                .w_4()
                                                                .h_4()
                                                                .mr_1()
                                                                .hover(|s| s.bg(theme.hover).rounded_md())
                                                                .on_mouse_down(MouseButton::Left, {
                                                                    let p = path.clone();
                                                                    let h = h_inner.clone();
                                                                    move |_, _, cx| {
                                                                        let _ = h.update(cx, |view, _| {
                                                                            view.state.toggle_sidebar_path(p.clone());
                                                                        });
                                                                    }
                                                                })
                                                                .child(svg().path(if *is_expanded { "icons/chevron-down.svg" } else { "icons/chevron-right.svg" }).text_color(theme.text_muted).size_full())
                                                        )
                                                        .child(
                                                            div()
                                                                .flex_1()
                                                                .flex()
                                                                .items_center()
                                                                .on_mouse_down(MouseButton::Left, {
                                                                    let p = path.clone();
                                                                    let h_inner_inner = h_inner.clone();
                                                                    move |e, _, cx| {
                                                                        if e.click_count == 2 {
                                                                            let _ = h_inner_inner.update(cx, |view, _| {
                                                                                view.state.toggle_sidebar_path(p.clone());
                                                                            });
                                                                        } else {
                                                                            let _ = h_inner_inner.update(cx, |this, cx| {
                                                                                let p_id = this.state.active_pane_id.clone();
                                                                                this.navigate_to(&p_id, p.clone(), cx);
                                                                            });
                                                                        }
                                                                    }
                                                                })
                                                                .on_mouse_down(MouseButton::Middle, {
                                                                    let p = path.clone();
                                                                    let h_inner_inner = h_inner.clone();
                                                                    move |_, _, cx| {
                                                                        let _ = h_inner_inner.update(cx, |view, cx| {
                                                                            let p_id = view.state.active_pane_id.clone();
                                                                            view.add_tab(&p_id, p.clone(), false, cx);
                                                                        });
                                                                    }
                                                                })
                                                                .child(
                                                                    div()
                                                                        .w_4()
                                                                        .h_4()
                                                                        .mr_2()
                                                                        .flex_shrink_0()
                                                                        .child(svg().path("icons/hard-drive.svg").text_color(theme.accent).size_full())
                                                                )
                                                                .child(
                                                                    div()
                                                                        .flex_1()
                                                                        .text_ellipsis()
                                                                        .overflow_hidden()
                                                                        .text_color(theme.text)
                                                                        .child(format!("{} ({})", drive.label, drive.path))
                                                                )
                                                        )
                                                        .on_mouse_down(MouseButton::Right, {
                                                            let h_right = h_inner.clone();
                                                            move |e, _, cx| {
                                                                let pos = e.position;
                                                                let _ = h_right.update(cx, |this, cx| {
                                                                    this.context_menu = Some((pos, ContextMenuContent::Items(super::oxyde_context_menu_items(h_right.clone()), None)));
                                                                    this.context_menu_scroll = UniformListScrollHandle::new();
                                                                    cx.notify();
                                                                });
                                                            }
                                                        })
                                                        .into_any_element()
                                                }
                                                SidebarItem::Directory { node, depth, is_expanded } => {
                                                    let path = node.path.clone();
                                                    let h_inner = h.clone();
                                                    div()
                                                        .h(px(SIDEBAR_ROW_H))
                                                        .pl(px(8.0 + (*depth as f32 * 12.0)))
                                                        .pr_3()
                                                        .rounded_md()
                                                        .flex()
                                                        .items_center()
                                                        .cursor_pointer()
                                                        .text_color(theme.text)
                                                        .hover(|s| s.bg(theme.hover))
                                                        .child(
                                                            div()
                                                                .w_4()
                                                                .h_4()
                                                                .mr_1()
                                                                .hover(|s| if node.has_subdirs { s.bg(theme.hover).rounded_md() } else { s })
                                                                .on_mouse_down(MouseButton::Left, {
                                                                    let p = path.clone();
                                                                    let has_subs = node.has_subdirs;
                                                                    let h_inner_inner = h_inner.clone();
                                                                    move |_, _, cx| {
                                                                        if has_subs {
                                                                            let _ = h_inner_inner.update(cx, |view, _| {
                                                                                view.state.toggle_sidebar_path(p.clone());
                                                                            });
                                                                        }
                                                                    }
                                                                })
                                                                .child(if node.has_subdirs {
                                                                    svg().path(if *is_expanded { "icons/chevron-down.svg" } else { "icons/chevron-right.svg" }).text_color(theme.text_muted).size_full()
                                                                } else {
                                                                    svg().path("")
                                                                })
                                                        )
                                                        .child(
                                                            div()
                                                                .flex_1()
                                                                .flex()
                                                                .items_center()
                                                                .on_mouse_down(MouseButton::Left, {
                                                                    let p = path.clone();
                                                                    let has_subs = node.has_subdirs;
                                                                    let h_inner_inner = h_inner.clone();
                                                                    move |e, _, cx| {
                                                                        if e.click_count == 2 && has_subs {
                                                                            let _ = h_inner_inner.update(cx, |view, _| {
                                                                                view.state.toggle_sidebar_path(p.clone());
                                                                            });
                                                                        } else {
                                                                            let _ = h_inner_inner.update(cx, |view, cx| {
                                                                                let p_id = view.state.active_pane_id.clone();
                                                                                view.navigate_to(&p_id, p.clone(), cx);
                                                                            });
                                                                        }
                                                                    }
                                                                })
                                                                .on_mouse_down(MouseButton::Middle, {
                                                                    let p = path.clone();
                                                                    let h_inner_inner = h_inner.clone();
                                                                    move |_, _, cx| {
                                                                        let _ = h_inner_inner.update(cx, |view, cx| {
                                                                            let p_id = view.state.active_pane_id.clone();
                                                                            view.add_tab(&p_id, p.clone(), false, cx);
                                                                        });
                                                                    }
                                                                })
                                                                .child(
                                                                    div()
                                                                        .w_4()
                                                                        .h_4()
                                                                        .mr_2()
                                                                        .flex_shrink_0()
                                                                        .child(svg().path("icons/folder.svg").text_color(rgb(0xe0b000)).size_full())
                                                                )
                                                                .child(
                                                                    div()
                                                                        .flex_1()
                                                                        .text_ellipsis()
                                                                        .overflow_hidden()
                                                                        .child(node.name.clone())
                                                                )
                                                        )
                                                        .on_mouse_down(MouseButton::Right, {
                                                            let h_right = h_inner.clone();
                                                            move |e, _, cx| {
                                                                let pos = e.position;
                                                                let _ = h_right.update(cx, |this, cx| {
                                                                    this.context_menu = Some((pos, ContextMenuContent::Items(super::oxyde_context_menu_items(h_right.clone()), None)));
                                                                    this.context_menu_scroll = UniformListScrollHandle::new();
                                                                    cx.notify();
                                                                });
                                                            }
                                                        })
                                                        .into_any_element()
                                                }
                                            }
                                        }).collect::<Vec<_>>()
                                    }
                                }
                            )
                            .track_scroll(view.sidebar_scroll.clone())
                            .size_full()
                        })
                )
        );

    // Vertical scrollbar
    container = container.child({
        let scroll_offset = -view.sidebar_scroll.0.borrow().base_handle.offset().y.as_f32();
        render_scrollbar(handle.clone(), ScrollbarAxis::Vertical, viewport_h, content_h, scroll_offset, matches!(view.drag_target, Some(DragTarget::SidebarV { .. })), "sidebar-v".to_string(), None, {
            move |this, track_id, v_h, c_h, _| {
                this.drag_target = Some(DragTarget::SidebarV { viewport_h: v_h, content_h: c_h, track_id });
            }
        })
    });

    // Horizontal scrollbar
    container = container.child(
        render_scrollbar(handle.clone(), ScrollbarAxis::Horizontal, viewport_w, content_w, view.sidebar_scroll_h.offset().x.as_f32(), matches!(view.drag_target, Some(DragTarget::SidebarH { .. })), "sidebar-h".to_string(), None, {
            move |this, track_id, v_w, c_w, _| {
                this.drag_target = Some(DragTarget::SidebarH { viewport_w: v_w, content_w: c_w, track_id });
            }
        })
    );

    // Resize handle
    container = container.child(
        div()
            .absolute()
            .top_0()
            .right_0()
            .bottom_0()
            .w(px(4.0))
            .cursor_col_resize()
            .on_mouse_down(MouseButton::Left, {
                let h = handle.clone();
                move |e, _, cx| {
                    let _ = h.update(cx, |this, _| {
                        this.drag_target = Some(DragTarget::SidebarResize);
                        this.grab_offset = e.position.x.as_f32() - this.sidebar_width;
                    });
                }
            })
    );

    container
}
