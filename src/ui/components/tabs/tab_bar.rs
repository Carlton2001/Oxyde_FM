use crate::ui::elements::bar_button;
use crate::ui::components::tooltip::NativeTooltipExt;
use crate::ui::scale::rem_pixels;
use crate::ui::style::h_flex;
use crate::ui::components::tooltip::Tooltip;
use crate::ui::i18n::{I18nContext, Message};
use gpui::prelude::*;
use gpui::*;


pub struct TabBar {
    id: ElementId,
    children: Vec<AnyElement>,
    scroll_state: crate::ui::workspace::TabStripState,
    on_new_tab: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
    /// Offset of the + button following the tabs, while a dragged tab opens or leaves room.
    new_tab_shift: f32,
}

impl TabBar {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            children: Vec::new(),
            scroll_state: Default::default(),
            on_new_tab: None,
            new_tab_shift: 0.0,
        }
    }

    pub fn new_tab_shift(mut self, shift: f32) -> Self {
        self.new_tab_shift = shift;
        self
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
            bar_button::fill(
                bar_button::cell(SharedString::from(format!("new-tab-btn-{:?}", self.id)), crate::ui::style::TAB_BAR_HEIGHT),
                true, false, *theme,
            )
            .bg(theme.header)
            .border_b(rems(0.0625))
            .border_color(theme.border)
            .fixed_tooltip(Tooltip::text(cx.i18n().text(Message::NewTab)))
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                on_new_tab(window, cx);
            })
            .child(bar_button::icon("icons/plus.svg", theme.text))
            .into_any_element()
        });
        let (inline_button, fixed_button) = if overflow {
            (None, new_tab_button)
        } else {
            let shift = self.new_tab_shift;
            let id = SharedString::from(format!("new-tab-shift-{:?}", self.id));
            (new_tab_button.map(|button| div().relative().flex_none().h_full().child(button)
                .with_spring(id, crate::ui::elements::motion::glide(shift), |button, x| button.left(px(x)))
                .into_any_element()), None)
        };
        let scroll_button = |id: &'static str, icon: &'static str, direction: f32| {
            let scroll = scroll.clone();
            let animation = animation.clone();
            let at_edge = if direction > 0.0 {
                scroll.offset().x >= rem_pixels(rems(-0.0625))
            } else {
                scroll.offset().x <= -scroll.max_offset().x + rem_pixels(rems(0.0625))
            };
            bar_button::fill(bar_button::cell(id, crate::ui::style::TAB_BAR_HEIGHT), !at_edge, false, *theme)
                .border_b(rems(0.0625))
                .border_color(theme.border)
                .when(!at_edge, |button| button.on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    cx.stop_propagation();
                    let step = (scroll.bounds().size.width * 0.75).max(rem_pixels(rems(5.0)));
                    super::scroll::smooth_scroll(animation.clone(), step * direction, window, cx);
                }))
                .child(bar_button::icon(icon, bar_button::icon_color(!at_edge, false, *theme)))
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
                d.child(scroll_button("tabs-left", "icons/chevron-left.svg", 1.0))
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
                d.child(scroll_button("tabs-right", "icons/chevron-right.svg", -1.0))
            })
            // Reserve a fixed button only while the tabs overflow.
            .children(fixed_button)
    }
}
