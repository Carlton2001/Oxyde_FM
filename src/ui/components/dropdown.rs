use gpui::*;
use gpui::prelude::*;
use crate::ui::app::OxydeApp;
use crate::ui::components::scrollbar::{render_scrollbar, ScrollbarAxis, ZED_TRACK_WIDTH};
use crate::ui::theme::{ThemeContext, PixelsExt};
use std::rc::Rc;

#[derive(Debug, PartialEq, Clone)]
pub enum IconName {
    FolderOpen,
    Copy,
    Minimize,
    Plus,
    ALargeSmall,
    Delete,
    Check,
    Square,
}

#[derive(Clone)]
pub struct DropdownItem {
    pub label: String,
    pub icon: Option<IconName>,
    pub stay_open: bool,
    pub on_click: Rc<dyn Fn(&mut Window, &mut App)>,
}

pub struct GenericDropdown {
    pub items: Vec<DropdownItem>,
    pub scroll_handle: UniformListScrollHandle,
    pub h_app: Entity<OxydeApp>,
    pub on_drag: Rc<dyn Fn(&mut OxydeApp, String, f32, f32, &mut App)>,
    pub is_dragging: bool,
    pub width: Option<f32>,
    pub max_height: f32,
    pub on_close: Rc<dyn Fn(&mut Window, &mut App)>,
    pub header: Option<AnyElement>,
}

impl Render for GenericDropdown {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.theme();
        let mut calculated_width: f32 = 180.0;
        
        let text_system = window.text_system();
        let font = window.text_style().font();
        
        for item in &self.items {
            let line = text_system.shape_line(
                item.label.clone().into(),
                px(13.0),
                &[TextRun {
                    len: item.label.len(),
                    font: font.clone(),
                    color: theme.text,
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                }],
                None,
            );
            calculated_width = calculated_width.max(line.width.as_f32() + 60.0);
        }

        let width = self.width.unwrap_or(calculated_width);
        let h_app = self.h_app.clone();
        let scroll_handle = self.scroll_handle.clone();
        let is_dragging = self.is_dragging;
        let on_drag = self.on_drag.clone();

        let item_count = self.items.len();
        let viewport_h = (item_count as f32 * 28.0).min(self.max_height);
        let content_h = item_count as f32 * 28.0;
        let has_scrollbar = content_h > viewport_h;

        let list_w = if has_scrollbar { width - ZED_TRACK_WIDTH } else { width };
        
        div()
            .occlude()
            .id("generic-dropdown")
            .on_mouse_move(|_, _, _| {})
            .flex_col()
            .p_2() // Global 8px margin inside the dropdown
            .w(px(width + 16.0)) // Account for padding in total width
            .bg(theme.surface)
            .border_1()
            .border_color(theme.border)
            .rounded_md()
            .shadow_lg()
            .when_some(self.header.take(), |s, h| s.child(div().child(h)))
            .child(
                div()
                    .w(px(list_w))
                    .h(px(viewport_h))
                    .child(
                        uniform_list("dropdown-items", item_count, {
                            let items = self.items.clone();
                            let on_close = self.on_close.clone();
                            let list_w_inner = list_w;
                            move |range, _, _| {
                                range.map(|ix| {
                                    let item = &items[ix];
                                    let on_click = item.on_click.clone();
                                    let on_close_inner = on_close.clone();
                                    let stay_open = item.stay_open;
                                    div()
                                        .w(px(list_w_inner))
                                        .h(px(28.0))
                                        .px_3()
                                        .flex()
                                        .items_center()
                                        .hover(|s| s.bg(theme.hover))
                                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                            (on_click)(window, cx);
                                            if !stay_open {
                                                (on_close_inner)(window, cx);
                                            }
                                        })
                                        .child(
                                            div()
                                                .w_4()
                                                .h_4()
                                                .mr_2()
                                                .child(if let Some(icon) = &item.icon {
                                                    svg().path(match icon {
                                                        IconName::FolderOpen => "icons/folder-open.svg",
                                                        IconName::Copy => "icons/copy.svg",
                                                        IconName::Minimize => "icons/minimize.svg",
                                                        IconName::Plus => "icons/plus.svg",
                                                        IconName::ALargeSmall => "icons/a-large-small.svg",
                                                        IconName::Delete => "icons/delete.svg",
                                                        IconName::Check => "icons/check.svg",
                                                        IconName::Square => "icons/square.svg",
                                                    }).size_full().text_color(theme.text_muted)
                                                } else {
                                                    svg().path("")
                                                })
                                        )
                                        .child(div().flex_1().text_color(theme.text).child(item.label.clone()))
                                }).collect::<Vec<_>>()
                            }
                        })
                        .track_scroll(scroll_handle.clone())
                        .size_full()
                    )
            )
            .when(has_scrollbar, |this| {
                this.child(
                    div()
                        .absolute()
                        .top_0()
                        .right_0()
                        .bottom_0()
                        .w(px(ZED_TRACK_WIDTH))
                        .on_mouse_down(MouseButton::Left, |_, _, _| {}) // Prevent click-through
                        .child({
                            let v_offset = -scroll_handle.0.borrow().base_handle.offset().y.as_f32();
                            render_scrollbar(h_app.clone(), ScrollbarAxis::Vertical, (viewport_h - 2.0).max(1.0), content_h, v_offset, is_dragging, "dropdown".to_string(), Some(20.0), {
                                let on_drag = on_drag.clone();
                                move |app, tid, vh, ch, cx| {
                                    (on_drag)(app, tid, vh, ch, cx);
                                }
                            })
                        })
                )
            })
            .on_mouse_down_out({
                let on_close = self.on_close.clone();
                move |_, window, cx| {
                    (on_close)(window, cx);
                }
            })
    }
}
