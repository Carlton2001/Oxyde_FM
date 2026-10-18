use crate::ui::components::tooltip::NativeTooltipExt;
use crate::ui::app::OxydeApp;
use crate::ui::components::tooltip::Tooltip;
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;

const BREATH: std::time::Duration = std::time::Duration::from_millis(2200);

/// Whether the halo breathes: while operations run, then until the current cycle completes.
fn breathing(
    state: &std::cell::Cell<Option<(std::time::Instant, Option<std::time::Instant>)>>,
    active: bool,
) -> bool {
    let now = std::time::Instant::now();
    let next = match (state.get(), active) {
        (None, false) => None,
        (None, true) => Some((now, None)),
        (Some((start, _)), true) => Some((start, None)),
        (Some((start, None)), false) => {
            let cycles = (now.duration_since(start).as_secs_f32() / BREATH.as_secs_f32()).ceil();
            Some((start, Some(start + BREATH.mul_f32(cycles))))
        }
        (Some((start, Some(end))), false) => (now < end).then_some((start, Some(end))),
    };
    state.set(next);
    next.is_some()
}

pub fn render_operations_footer(
    app: &OxydeApp,
    handle: Entity<OxydeApp>,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let theme = *cx.theme();
    let (total, active) = app.file_ops.operation_counts();
    let enabled = total > 0;
    let tooltip = cx.i18n().text(Message::OpsShowPanel);
    let breathing = breathing(&app.operations_breath, active > 0);
    div()
        .id("title-operations")
        .w(rems(2.25))
        .h_full()
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .gap(rems(0.375))
        .px(rems(0.5))
        .rounded_sm()
        .text_color(theme.text_muted)
        .occlude()
        .native_tooltip(Tooltip::text(tooltip))
        .when(!enabled, |button| button.opacity(0.4))
        .when(enabled, |button| {
            button.hover(|style| style.bg(theme.control_hover).text_color(theme.text))
        })
        .child({
            let icon = div().size_4().rounded_full().child(svg()
                .path("icons/download.svg")
                .text_color(if active > 0 { theme.accent } else { theme.text })
                .size_4());
            let halo = move |breath: f32| vec![BoxShadow {
                inset: false,
                color: theme.accent.opacity(0.2 + 0.5 * breath),
                offset: point(px(0.0), px(0.0)),
                blur_radius: crate::ui::scale::rem_pixels(rems(0.375 + 0.375 * breath)),
                spread_radius: crate::ui::scale::rem_pixels(rems(0.0625 + 0.125 * breath)),
            }];
            if breathing {
                icon.with_animation(
                    "operations-breathing",
                    Animation::new(BREATH).repeat(),
                    move |icon, delta| icon.shadow(halo((1.0 - (delta * std::f32::consts::TAU).cos()) * 0.5)),
                )
                .into_any_element()
            } else {
                icon.into_any_element()
            }
        })
        .when(enabled, |button| {
            button.on_mouse_down(MouseButton::Left, move |_, _, cx| {
                super::open_operations_window(handle.clone(), cx);
            })
        })
}
