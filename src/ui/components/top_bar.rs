use gpui::*;
use gpui::prelude::*;
use crate::ui::app::OxydeApp;
// use gpui_component::{button::{Button, ButtonVariants}, IconName, Size as ComponentSize, Sizable};
use crate::ui::components::dropdown::IconName;

use oxyde_lib::models::FileEntry;

pub fn render_top_bar(view: &OxydeApp, pane_id: String, handle: Entity<OxydeApp>, _cx: &mut Context<OxydeApp>) -> impl IntoElement {
    let pane = view.state.layout.find_pane(&pane_id).expect("pane exists");
    let active_tab = pane.active_tab();
    let current_path = active_tab.path.clone();

    let path_parts: Vec<&str> = current_path
        .split(|c| c == '/' || c == '\\')
        .filter(|s: &&str| !s.is_empty())
        .collect();

    let can_go_back = !active_tab.back_stack.is_empty();
    let can_go_forward = !active_tab.forward_stack.is_empty();

    div()
        .h(px(32.0))
        .w_full()
        .flex()
        .items_center()
        .px_4()
        .bg(rgb(0x0c0c0c))
        .border_b_1()
        .border_color(rgb(0x1f1f1f))
        .child(
            div()
                .flex()
                .items_center()
                .child({
                    let mut btn = div()
                        .w_6()
                        .h_6()
                        .mr_2()
                        .flex()
                        .items_center()
                        .justify_center();

                    if can_go_back {
                        let p_id = pane_id.clone();
                        btn = btn
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0x1a1a1a)).rounded_md())
                            .on_mouse_down(MouseButton::Left, {
                                let h = handle.clone();
                                move |_, _, cx| { let _ = h.update(cx, |v: &mut OxydeApp, cx| v.go_back(&p_id, cx)); }
                            });
                    }

                    btn.child(
                        svg()
                            .path("icons/arrow-left.svg")
                            .text_color(if can_go_back { rgb(0x888888) } else { rgb(0x333333) })
                            .size_4()
                    )
                })
                .child({
                    let mut btn = div()
                        .w_6()
                        .h_6()
                        .mr_4()
                        .flex()
                        .items_center()
                        .justify_center();

                    if can_go_forward {
                        let p_id = pane_id.clone();
                        btn = btn
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0x1a1a1a)).rounded_md())
                            .on_mouse_down(MouseButton::Left, {
                                let h = handle.clone();
                                move |_, _, cx| { let _ = h.update(cx, |v: &mut OxydeApp, cx| v.go_forward(&p_id, cx)); }
                            });
                    }

                    btn.child(
                        svg()
                            .path("icons/arrow-right.svg")
                            .text_color(if can_go_forward { rgb(0x888888) } else { rgb(0x333333) })
                            .size_4()
                    )
                })
        )
        .child(
            div()
                .flex()
                .items_center()
                .text_color(rgb(0x666666))
                .children({
                    let mut accumulated_path = String::new();
                    let mut elements = Vec::new();

                    for (index, part) in path_parts.iter().enumerate() {
                        let p_id = pane_id.clone();
                        if index == 0 && part.ends_with(':') {
                            accumulated_path = format!("{}/", part);
                        } else {
                            if !accumulated_path.is_empty() && !accumulated_path.ends_with('/') {
                                accumulated_path.push('/');
                            }
                            accumulated_path.push_str(part);
                        }

                        let level_path = accumulated_path.clone();

                        if index > 0 {
                             let mut parent_path = level_path.clone();
                             if parent_path.ends_with(part) {
                                  parent_path = parent_path[..parent_path.len() - part.len()].to_string();
                             }

                             let parent_path_cap = parent_path.clone();

                             elements.push(
                                div()
                                    .relative()
                                    .child(
                                        div()
                                            .id(SharedString::from(format!("chevron-btn-{}-{}", p_id, index)))
                                            .cursor_pointer()
                                            .hover(|s| s.bg(rgb(0x1a1a1a)).rounded_md())
                                            .px_1()
                                            .mx_0()
                                            .child(svg().path("icons/chevron-right.svg").size_4().text_color(rgb(0x888888)))
                                            .on_mouse_down(MouseButton::Left, {
                                                let handle = handle.clone();
                                                let p_id_cap = p_id.clone();
                                                let path_cap = parent_path_cap.clone();
                                                let idx_cap = index;
                                                move |_, _, cx: &mut App| {
                                                    let handle = handle.clone();
                                                    let p_id_cap = p_id_cap.clone();
                                                    let path_cap = path_cap.clone();
                                                    cx.spawn(move |cx: &mut AsyncApp| {
                                                        let cx = cx.clone();
                                                        let handle = handle.clone();
                                                        let p_id_cap = p_id_cap.clone();
                                                        let path_cap = path_cap.clone();
                                                        let idx_cap = idx_cap;
                                                        async move {
                                                            if let Ok(dirs) = oxyde_lib::commands::io::list_dir(path_cap.clone(), None, None, None, None).await {
                                                                 let _ = cx.update(|cx| {
                                                                     let _ = handle.update(cx, |this, cx| {
                                                                         let filtered_entries: Vec<FileEntry> = dirs.entries.into_iter().filter(|e| e.is_dir).collect();
                                                                         this.breadcrumb_menu = Some((p_id_cap, path_cap, idx_cap, filtered_entries));
                                                                         cx.notify();
                                                                     });
                                                                 });
                                                            }
                                                        }
                                                    }).detach();
                                                }
                                            })
                                    )
                                    .child({
                                        let breadcrumb_menu = view.breadcrumb_menu.clone();
                                        div()
                                             .when_some(breadcrumb_menu, |this: Div, (m_pane_id, menu_path, _, dirs): (String, String, usize, Vec<FileEntry>)| {
                                                 if m_pane_id == p_id && menu_path == parent_path_cap {
                                                     let items = dirs.into_iter()
                                                         .map(|entry| {
                                                         let h = handle.clone();
                                                         let p_id = p_id.clone();
                                                         let entry_path = entry.path.clone();
                                                         crate::ui::components::dropdown::DropdownItem {
                                                             label: entry.name.clone(),
                                                             icon: Some(IconName::FolderOpen),
                                                             stay_open: false,
                                                             on_click: std::rc::Rc::new(move |_, app| {
                                                                 let _ = h.update(app, |this, cx| {
                                                                     this.navigate_to(&p_id, entry_path.clone(), cx);
                                                                     this.breadcrumb_menu = None;
                                                                 });
                                                             }),
                                                         }
                                                     }).collect();

                                                     this.child(
                                                         deferred(
                                                             anchored()
                                                                 .anchor(Corner::BottomLeft)
                                                                 .child(
                                                                     _cx.new(|_| crate::ui::components::dropdown::GenericDropdown {
                                                                         h_app: handle.clone(),
                                                                         items,
                                                                         scroll_handle: view.breadcrumb_scroll.clone(),
                                                                         width: None,
                                                                         max_height: 360.0,
                                                                         header: None,
                                                                         on_close: {
                                                                             let h = handle.clone();
                                                                             std::rc::Rc::new(move |_, app| {
                                                                                 let _ = h.update(app, |this, _| {
                                                                                     this.breadcrumb_menu = None;
                                                                                 });
                                                                             })
                                                                         },
                                                                         on_drag: std::rc::Rc::new(|this, tid, vh, ch, _| {
                                                                            this.drag_target = Some(crate::ui::app::DragTarget::BreadcrumbV { 
                                                                                viewport_h: vh, 
                                                                                content_h: ch, 
                                                                                track_id: tid 
                                                                            });
                                                                         }),
                                                                         is_dragging: matches!(view.drag_target, Some(crate::ui::app::DragTarget::BreadcrumbV { .. })),
                                                                     })
                                                                 )
                                                         )
                                                     )
                                                 } else {
                                                     this
                                                 }
                                             })
                                    })
                                    .into_any_element()
                             );
                        }

                        let target_path = level_path.clone();
                        elements.push(
                            div()
                                .cursor_pointer()
                                .hover(|s| s.text_color(rgb(0xdddddd)))
                                .on_mouse_down(MouseButton::Left, {
                                    let h = handle.clone();
                                    let p_id_cap = p_id.clone();
                                    move |_, _, cx| {
                                        let _ = h.update(cx, |this: &mut OxydeApp, cx| {
                                            this.navigate_to(&p_id_cap, target_path.clone(), cx);
                                        });
                                    }
                                })
                                .child(part.to_string())
                                .into_any_element()
                        );
                    }
                    elements
                })
        )
}
