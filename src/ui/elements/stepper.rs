//! The − value + control, in the frame of [`super::segmented`].
//! The caller owns the value, its bounds, its default and persistence, and the keyboard.

use crate::ui::components::tooltip::{NativeTooltipExt, Tooltip};
use crate::ui::style::TabularFigures;
use crate::ui::theme::OxydeTheme;
use gpui::prelude::*;
use gpui::*;
use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

/// A held button steps once, waits this long, then repeats, as GTK's spin buttons.
const REPEAT_DELAY: Duration = Duration::from_millis(400);
const REPEAT_INTERVAL: Duration = Duration::from_millis(80);

pub(crate) type Step = Rc<dyn Fn(bool, &mut Window, &mut App)>;

/// The press being repeated, and what the last render allowed.
#[derive(Default)]
struct Hold {
    /// Bumped by every press and release; a repeat only runs for the press that started it.
    press: Cell<u64>,
    held: Cell<bool>,
    can_decrease: Cell<bool>,
    can_increase: Cell<bool>,
}

impl Hold {
    fn allows(&self, increase: bool) -> bool {
        if increase { self.can_increase.get() } else { self.can_decrease.get() }
    }

    fn release(&self) {
        self.held.set(false);
        self.press.set(self.press.get() + 1);
    }
}

#[derive(IntoElement)]
pub(crate) struct Stepper {
    id: &'static str,
    value: SharedString,
    can_decrease: bool,
    can_increase: bool,
    reset: Option<(SharedString, Rc<dyn Fn(&mut Window, &mut App)>)>,
    theme: OxydeTheme,
    on_step: Step,
}

/// `value` is shown between the buttons; `on_step` receives `true` to increase.
pub(crate) fn stepper(
    id: &'static str,
    value: impl Into<SharedString>,
    can_decrease: bool,
    can_increase: bool,
    theme: OxydeTheme,
    on_step: Step,
) -> Stepper {
    Stepper { id, value: value.into(), can_decrease, can_increase, reset: None, theme, on_step }
}

impl Stepper {
    /// A click on the value restores the default, explained by `tooltip`.
    pub(crate) fn reset(mut self, tooltip: SharedString, reset: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.reset = Some((tooltip, Rc::new(reset)));
        self
    }
}

impl RenderOnce for Stepper {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Stepper { id, value, can_decrease, can_increase, reset, theme, on_step } = self;
        stepper_element(id, value, can_decrease, can_increase, reset, theme, on_step, window, cx)
    }
}

#[allow(clippy::too_many_arguments)]
fn stepper_element(
    id: &'static str,
    value: SharedString,
    can_decrease: bool,
    can_increase: bool,
    reset: Option<(SharedString, Rc<dyn Fn(&mut Window, &mut App)>)>,
    theme: OxydeTheme,
    on_step: Step,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let hold = window.use_keyed_state(SharedString::from(format!("{id}-hold")), cx, |_, _| Rc::new(Hold::default()))
        .read(cx).clone();
    hold.can_decrease.set(can_decrease);
    hold.can_increase.set(can_increase);
    let button = |increase: bool, enabled: bool| {
        let (on_step, hold, release, release_out) = (on_step.clone(), hold.clone(), hold.clone(), hold.clone());
        super::segmented::segment(
                SharedString::from(format!("{id}-{}", if increase { "increase" } else { "decrease" })),
                false,
                enabled,
                theme,
            )
            .w(rems(1.5))
            .px_0()
            .child(svg().path(if increase { "icons/plus.svg" } else { "icons/minus.svg" }).size(rems(0.875)).flex_none()
                .text_color(if enabled { theme.text } else { theme.icon_disabled }))
            .when(enabled, |button| button.on_mouse_down(MouseButton::Left, move |_, window, cx| {
                cx.stop_propagation();
                on_step(increase, window, cx);
                hold.release();
                hold.held.set(true);
                let press = hold.press.get();
                let (hold, on_step) = (hold.clone(), on_step.clone());
                window.spawn(cx, async move |cx| {
                    let mut wait = REPEAT_DELAY;
                    loop {
                        cx.background_executor().timer(wait).await;
                        wait = REPEAT_INTERVAL;
                        if !hold.held.get() || hold.press.get() != press || !hold.allows(increase) {
                            break;
                        }
                        if cx.update(|window, cx| on_step(increase, window, cx)).is_err() {
                            break;
                        }
                    }
                }).detach();
            }))
            .on_mouse_up(MouseButton::Left, move |_, _, _| release.release())
            .on_mouse_up_out(MouseButton::Left, move |_, _, _| release_out.release())
    };
    let value = div()
        .id(SharedString::from(format!("{id}-value")))
        .min_w(rems(3.25))
        .h(super::segmented::SEGMENT_HEIGHT)
        .px(rems(0.375))
        .flex()
        .items_center()
        .justify_center()
        .text_color(theme.text)
        .tabular_figures()
        .child(value)
        .when_some(reset, |value, (tooltip, reset)| {
            value
                .hover(move |style| style.bg(super::segmented::hover_fill(theme)))
                .fixed_tooltip(Tooltip::text(tooltip))
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    cx.stop_propagation();
                    reset(window, cx);
                })
        });
    super::segmented::segmented(SharedString::from(format!("{id}-stepper")), None, theme)
        .child(button(false, can_decrease))
        .child(value)
        .child(button(true, can_increase))
}
