use crate::ui::scale::rem_pixels;
use crate::ui::style::h_flex;
use gpui::prelude::*;
use gpui::*;

fn tab_bar_button(id: impl Into<ElementId>, theme: &crate::ui::theme::OxydeTheme) -> Stateful<Div> {
    let hover = theme.hover;
    div()
        .id(id)
        .flex_none()
        .size(rems(1.5))
        .mx(rems(0.25))
        .my_auto()
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .hover(move |s| s.bg(hover))
}

pub struct TabBar {
    id: ElementId,
    start_children: Vec<AnyElement>,
    children: Vec<AnyElement>,
    end_children: Vec<AnyElement>,
    scroll_state: crate::ui::workspace::TabStripState,
    on_new_tab: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
    drop_target_indicator: bool,
}

impl TabBar {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            start_children: Vec::new(),
            children: Vec::new(),
            end_children: Vec::new(),
            scroll_state: Default::default(),
            on_new_tab: None,
            drop_target_indicator: false,
        }
    }

    pub fn on_new_tab(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_new_tab = Some(Box::new(f));
        self
    }

    pub fn scroll_state(
        mut self,
        state: crate::ui::workspace::TabStripState,
        active_id: String,
        active_index: usize,
    ) -> Self {
        if *state.active_id.borrow() != active_id || state.active_index.get() != active_index {
            *state.active_id.borrow_mut() = active_id;
            state.active_index.set(active_index);
            state.scroll.scroll_to_item(active_index);
        }
        self.scroll_state = state;
        self
    }

    pub fn drop_target_indicator(mut self, show: bool) -> Self {
        self.drop_target_indicator = show;
        self
    }

    #[allow(dead_code)]
    pub fn start_child(mut self, child: impl IntoElement) -> Self {
        self.start_children.push(child.into_any_element());
        self
    }

    #[allow(dead_code)]
    pub fn end_child(mut self, child: impl IntoElement) -> Self {
        self.end_children.push(child.into_any_element());
        self
    }
}

impl ParentElement for TabBar {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl IntoElement for TabBar {
    type Element = ViewElement<Self>;

    fn into_element(self) -> Self::Element {
        ViewElement::new(self)
    }
}

impl RenderOnce for TabBar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        use crate::ui::theme::ThemeContext;
        let theme = cx.theme();
        let overflow = self.scroll_state.overflow.get();
        let scroll = self.scroll_state.scroll;
        let wheel_scroll = scroll.clone();
        let measure_scroll = scroll.clone();
        let overflow_state = self.scroll_state.overflow;
        let active_index = self.scroll_state.active_index.get();
        let scroll_button = |id: &'static str, icon: &'static str, direction: f32| {
            let scroll = scroll.clone();
            let at_edge = if direction > 0.0 {
                scroll.offset().x >= rem_pixels(rems(-0.0625))
            } else {
                scroll.offset().x <= -scroll.max_offset().x + rem_pixels(rems(0.0625))
            };
            tab_bar_button(id, theme)
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    cx.stop_propagation();
                    let step = (scroll.bounds().size.width * 0.75).max(rem_pixels(rems(5.0)));
                    let x = (scroll.offset().x + step * direction)
                        .clamp(-scroll.max_offset().x, px(0.0));
                    scroll.set_offset(point(x, px(0.0)));
                    window.refresh();
                })
                .child(
                    svg()
                        .path(icon)
                        .size_4()
                        .text_color(theme.text_muted)
                        .when(at_edge, |s| s.opacity(0.4)),
                )
        };

        div()
            .id(self.id.clone())
            .relative()
            .flex()
            .flex_none()
            .w_full()
            .min_w_0()
            .overflow_hidden()
            .h(rems(2.0))
            .bg(theme.header)
            // Paint behind the tabs so the active surface covers the separator.
            .child(
                div()
                    .absolute()
                    .bottom_0()
                    .w_full()
                    .h(rems(0.0625))
                    .bg(theme.border),
            )
            .child(h_flex().flex_none().children(self.start_children))
            .when(overflow, |d| {
                d.child(scroll_button("tabs-left", "icons/arrow-left.svg", 1.0))
            })
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .overflow_hidden()
                    .child(
                        h_flex()
                            .id("tabs-scroll")
                            .w_full()
                            .min_w_0()
                            .h_full()
                            .overflow_x_scroll()
                            .track_scroll(&scroll)
                            .on_scroll_wheel(move |event, window, cx| {
                                let delta = event.delta.pixel_delta(window.line_height());
                                let amount = if delta.y.abs() > delta.x.abs() {
                                    delta.y
                                } else {
                                    delta.x
                                };
                                let max = wheel_scroll.max_offset().x;
                                if max > px(0.0) && amount != px(0.0) {
                                    let x = (wheel_scroll.offset().x + amount * 1.5)
                                        .clamp(-max, px(0.0));
                                    wheel_scroll.set_offset(point(x, px(0.0)));
                                    window.refresh();
                                    cx.stop_propagation();
                                }
                            })
                            .children(self.children)
                            // Small gap for drop target at the end of tabs list
                            .child(
                                div()
                                    .w(rems(0.25)) // Reduced from 8px
                                    .flex_none()
                                    .h_full()
                                    .relative()
                                    .child(
                                        div()
                                            .absolute()
                                            .top_0()
                                            .left_0()
                                            .w(rems(0.125))
                                            .h_full()
                                            .bg(theme.accent)
                                            .when(!self.drop_target_indicator, |s| s.opacity(0.0)),
                                    ),
                            ),
                    )
                    .child(
                        canvas(
                            |bounds, _, _| bounds,
                            move |_, _: Bounds<Pixels>, window, _| {
                                // Account for the arrows when deciding whether the unadorned strip fits.
                                let max = measure_scroll.max_offset().x;
                                let needs_arrows = max
                                    > rem_pixels(rems(if overflow {
                                        65.0 / 16.0
                                    } else {
                                        1.0 / 16.0
                                    }));
                                if overflow_state.replace(needs_arrows) != needs_arrows {
                                    measure_scroll.scroll_to_item(active_index);
                                    window.refresh();
                                }
                            },
                        )
                        .absolute()
                        .size_full(),
                    ),
            )
            .when(overflow, |d| {
                d.child(scroll_button("tabs-right", "icons/arrow-right.svg", -1.0))
            })
            // Keep the new-tab button outside the clipped, scrolling strip.
            .when_some(self.on_new_tab, |this, on_new_tab| {
                this.child(
                    tab_bar_button(
                        SharedString::from(format!("new-tab-btn-{:?}", self.id)),
                        theme,
                    )
                    .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                        on_new_tab(window, cx);
                    })
                    .child(
                        svg()
                            .path("icons/plus.svg")
                            .size_4()
                            .text_color(theme.text_muted),
                    ),
                )
            })
            .child(h_flex().flex_none().children(self.end_children))
    }
}
