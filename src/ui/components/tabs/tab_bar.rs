use crate::ui::scale::rem_pixels;
use crate::ui::style::h_flex;
use crate::ui::components::tooltip::Tooltip;
use crate::ui::i18n::{I18nContext, Message};
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
    children: Vec<AnyElement>,
    scroll_state: crate::ui::workspace::TabStripState,
    on_new_tab: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
}

impl TabBar {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            children: Vec::new(),
            scroll_state: Default::default(),
            on_new_tab: None,
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
            state.animation_generation.set(state.animation_generation.get().wrapping_add(1));
            state.animation_target.set(None);
            *state.active_id.borrow_mut() = active_id;
            state.active_index.set(active_index);
            state.scroll.scroll_to_item(active_index);
        }
        self.scroll_state = state;
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
        let animation = self.scroll_state.clone();
        let wheel_animation = animation.clone();
        let scroll = self.scroll_state.scroll;
        let wheel_scroll = scroll.clone();
        let measure_scroll = scroll.clone();
        let overflow_state = self.scroll_state.overflow;
        let active_index = self.scroll_state.active_index.get();
        let new_tab_button = self.on_new_tab.map(|on_new_tab| {
            div()
            .id(SharedString::from(format!("new-tab-btn-{:?}", self.id)))
            .flex_none()
            .w(rems(2.0))
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme.header)
            .border_b(rems(0.0625))
            .border_color(theme.border)
            .hover(|s| s.bg(theme.hover))
            .tooltip(Tooltip::text(cx.i18n().text(Message::NewTab)))
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                on_new_tab(window, cx);
            })
            .child(svg().path("icons/plus.svg").size_4().text_color(theme.text_muted))
            .into_any_element()
        });
        let (inline_button, fixed_button) = if overflow {
            (None, new_tab_button)
        } else {
            (new_tab_button, None)
        };
        let scroll_button = |id: &'static str, icon: &'static str, direction: f32| {
            let scroll = scroll.clone();
            let animation = animation.clone();
            let at_edge = if direction > 0.0 {
                scroll.offset().x >= rem_pixels(rems(-0.0625))
            } else {
                scroll.offset().x <= -scroll.max_offset().x + rem_pixels(rems(0.0625))
            };
            tab_bar_button(id, theme)
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    cx.stop_propagation();
                    let step = (scroll.bounds().size.width * 0.75).max(rem_pixels(rems(5.0)));
                    super::scroll::smooth_scroll(animation.clone(), step * direction, window, cx);
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
            .h(crate::ui::style::TAB_BAR_HEIGHT)
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
                                    super::scroll::smooth_scroll(wheel_animation.clone(), amount * 1.5, window, cx);
                                    cx.stop_propagation();
                                }
                            })
                            .children(self.children)
                            .children(inline_button)
                            // Trailing spacing in the scrollable strip.
                            .child(
                                div()
                                    .w(rems(0.25))
                                    .flex_none()
                                    .h_full(),
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
            // Reserve a fixed button only while the tabs overflow.
            .children(fixed_button)
    }
}
