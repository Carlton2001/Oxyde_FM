use gpui::*;
use gpui::prelude::*;
use crate::ui::app::OxydeApp;
use crate::ui::components::scrollbar::{render_scrollbar, ScrollbarAxis, ZED_TRACK_WIDTH};
use crate::ui::theme::{PixelsExt};
use std::rc::Rc;

#[derive(Clone)]
pub enum IconName {
    Plus,
    ALargeSmall,
    Delete,
    Check,
    Square,
    X,
    FolderOpen,
    HardDrive,
    Copy,
    Minimize,
}

#[derive(Clone)]
pub struct DropdownItem {
    pub label: String,
    pub icon: Option<IconName>,
    pub stay_open: bool,
    pub has_submenu: bool,
    pub on_click: Rc<dyn Fn(&mut Window, &mut App)>,
    pub on_hover: Option<Rc<dyn Fn(Point<Pixels>, bool, &mut App)>>,
}

#[allow(dead_code)]
impl DropdownItem {
    pub fn new(label: impl Into<String>, on_click: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        Self {
            label: label.into(),
            icon: None,
            stay_open: false,
            has_submenu: false,
            on_click: Rc::new(on_click),
            on_hover: None,
        }
    }

    pub fn on_hover(mut self, f: impl Fn(Point<Pixels>, bool, &mut App) + 'static) -> Self {
        self.on_hover = Some(Rc::new(f));
        self
    }

    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn stay_open(mut self, stay_open: bool) -> Self {
        self.stay_open = stay_open;
        self
    }

    pub fn has_submenu(mut self, has_submenu: bool) -> Self {
        self.has_submenu = has_submenu;
        self
    }
}

pub struct GenericDropdown {
    pub items: Vec<DropdownItem>,
    pub scroll_handle: UniformListScrollHandle,
    pub h_app: Entity<OxydeApp>,
    pub on_drag: Rc<dyn Fn(&mut OxydeApp, String, f32, f32, &mut App)>,
    pub on_close: Rc<dyn Fn(&mut Window, &mut App)>,
    pub is_dragging: bool,
    pub width: Option<f32>,
    pub max_height: f32,
    pub header: Option<AnyElement>,
    pub on_item_hover: Option<Rc<dyn Fn(usize, Point<Pixels>, &mut App)>>,
    pub on_width_calculated: Option<Rc<dyn Fn(Pixels, &mut App)>>,
    /// Fired with `true` when mouse enters, `false` when mouse leaves.
    pub on_hover_change: Option<Rc<dyn Fn(bool, &mut App)>>,
}

impl Render for GenericDropdown {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use crate::ui::theme::OxydeTheme;
        let theme = *OxydeTheme::global(cx);

        // Menu-specific colors — richer and more visible than the generic theme values
        let menu_bg: Hsla        = rgb(0x1e1f23).into();
        let menu_border: Hsla    = rgba(0xffffff14).into(); // ~8% white — subtle but visible
        let item_hover: Hsla     = rgba(0xffffff12).into(); // ~7% white on item hover
        let icon_color: Hsla     = rgba(0xffffff55).into(); // ~33% white for icons
        let chevron_color: Hsla  = rgba(0xffffff33).into(); // ~20% white for chevron arrow

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
            // icon(16) + gap(8) + text + chevron(16) + item-px(8*2) + container-px(4*2)
            calculated_width = calculated_width.max(line.width.as_f32() + 60.0);
        }

        let width = self.width.unwrap_or(calculated_width);

        // Report computed width (including 8px horizontal container padding)
        if let Some(on_width) = self.on_width_calculated.take() {
            (on_width)(px(width + 8.0), cx);
        }

        let h_app_copy = self.h_app.clone();
        let scroll_handle = self.scroll_handle.clone();
        let is_dragging = self.is_dragging;
        let on_drag = self.on_drag.clone();

        let item_h: f32 = 26.0;
        let item_count = self.items.len();
        let viewport_h = (item_count as f32 * item_h).min(self.max_height);
        let content_h = item_count as f32 * item_h;
        let has_scrollbar = content_h > viewport_h;

        let list_w = if has_scrollbar { width - ZED_TRACK_WIDTH } else { width };

        let on_item_hover = self.on_item_hover.clone();

        div()
            .occlude()
            .id("generic-dropdown")
            .on_mouse_move(|_, _, _| {})
            .flex_col()
            // 4px vertical padding; 4px horizontal so item hover doesn't bleed to edge
            .py(px(4.0))
            .px(px(4.0))
            .w(px(width + 8.0))
            .bg(menu_bg)
            .border_1()
            .border_color(menu_border)
            .rounded(px(8.0))
            .shadow_lg()
            .when_some(self.header.take(), |s, h| s.child(div().mb(px(2.0)).child(h)))
            .child(
                div()
                    .w(px(list_w))
                    .h(px(viewport_h))
                    .child(
                        uniform_list("dropdown-items", item_count, {
                            let items = self.items.clone();
                            let on_close = self.on_close.clone();
                            let list_w_inner = list_w;
                            let on_hover_list = on_item_hover.clone();
                            move |range, _, _| {
                                range.map(|ix| {
                                    let item = &items[ix];
                                    let on_click = item.on_click.clone();
                                    let on_close_inner = on_close.clone();
                                    let stay_open = item.stay_open;
                                    let on_h = on_hover_list.clone();

                                    div()
                                        .w(px(list_w_inner))
                                        .h(px(item_h))
                                        .px(px(8.0))
                                        .flex()
                                        .items_center()
                                        .rounded(px(5.0))
                                        .hover(|s| s.bg(item_hover))
                                        .cursor_pointer()
                                        .child(
                                            canvas(
                                                |bounds, _, _| bounds,
                                                move |bounds, _, window, cx| {
                                                    if let Some(on_h) = &on_h {
                                                        if bounds.contains(&window.mouse_position()) {
                                                            (on_h)(ix, bounds.origin, cx);
                                                        }
                                                    }
                                                }
                                            ).absolute().size_full()
                                        )
                                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                            (on_click)(window, cx);
                                            if !stay_open {
                                                (on_close_inner)(window, cx);
                                            }
                                        })
                                        // Icon slot — always 16px wide for alignment
                                        .child(
                                            div()
                                                .flex_none()
                                                .w(px(16.0))
                                                .h(px(16.0))
                                                .mr(px(8.0))
                                                .when_some(item.icon.as_ref(), |d, icon| {
                                                    d.child(
                                                        svg().path(match icon {
                                                            IconName::Plus        => "icons/plus.svg",
                                                            IconName::ALargeSmall => "icons/a-large-small.svg",
                                                            IconName::Delete      => "icons/delete.svg",
                                                            IconName::Check       => "icons/check.svg",
                                                            IconName::Square      => "icons/square.svg",
                                                            IconName::X           => "icons/x.svg",
                                                            IconName::FolderOpen  => "icons/folder-open.svg",
                                                            IconName::HardDrive   => "icons/hard-drive.svg",
                                                            IconName::Copy        => "icons/copy.svg",
                                                            IconName::Minimize    => "icons/minimize.svg",
                                                        })
                                                        .size_full()
                                                        .text_color(icon_color)
                                                    )
                                                })
                                        )
                                        // Label
                                        .child(
                                            div()
                                                .flex_1()
                                                .text_size(px(13.0))
                                                .text_color(theme.text)
                                                .child(item.label.clone())
                                        )
                                        // Submenu arrow
                                        .when(item.has_submenu, |s| {
                                            s.child(
                                                svg()
                                                    .path("icons/chevron-right.svg")
                                                    .w(px(14.0))
                                                    .h(px(14.0))
                                                    .text_color(chevron_color)
                                            )
                                        })
                                }).collect::<Vec<_>>()
                            }
                        })
                        .track_scroll(scroll_handle.clone())
                        .size_full()
                    )
            )
            .child({
                let current_scroll = scroll_handle.0.borrow().base_handle.offset().y.as_f32().abs();
                render_scrollbar(
                    h_app_copy,
                    ScrollbarAxis::Vertical,
                    viewport_h,
                    content_h,
                    current_scroll,
                    is_dragging,
                    "context-menu-scrollbar".to_string(),
                    None,
                    move |this, tid, vh, ch, cx| { (on_drag)(this, tid, vh, ch, cx); }
                )
            })
            .on_mouse_down_out({
                let on_close = self.on_close.clone();
                move |_, window, cx| {
                    (on_close)(window, cx);
                }
            })
            .when_some(self.on_hover_change.clone(), |d, cb| {
                d.on_hover(move |is_hovering, _, cx| { (cb)(*is_hovering, cx); })
            })
    }
}
