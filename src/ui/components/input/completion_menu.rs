use super::TextInput;
use crate::ui::theme::ThemeContext;
use gpui::{prelude::*, *};

pub(super) fn render(
    view: &TextInput,
    window: &mut Window,
    cx: &mut Context<TextInput>,
) -> AnyElement {
    let theme = *cx.theme();
    let rows = view
        .completions
        .iter()
        .enumerate()
        .map(|(index, path)| {
            let path = path.clone();
            let selected = index == view.completion_index;
            let handle = cx.entity();
            div()
                .id(("path-completion", index))
                .px_3()
                .h(rems(1.75))
                .flex()
                .items_center()
                .text_size(rems(0.8125))
                .text_color(theme.text)
                .when(selected, |row| row.bg(theme.selected))
                .hover(|style| style.bg(theme.control_hover))
                .child(div().min_w_0().truncate().child(path.clone()))
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    cx.stop_propagation();
                    crate::ui::components::popup::with_owner(window, cx, |window, cx| {
                        handle.update(cx, |this, cx| {
                            this.accept_completion(path.clone(), window, cx)
                        })
                    });
                })
        })
        .collect::<Vec<_>>();
    let measure = cx.entity();
    let menu = div()
        .id("path-completion-list")
        .when(crate::ui::components::popup::is_menu(window, cx), |menu| {
            menu.w(view
                .completion_anchor
                .map(|bounds| bounds.size.width)
                .unwrap_or(rems(20.0).to_pixels(window.rem_size())))
        })
        .when(!crate::ui::components::popup::is_menu(window, cx), |menu| {
            menu.w_full()
        })
        .max_h(crate::ui::components::popup::available_size(window, cx).height)
        .overflow_y_scroll()
        .track_scroll(&view.completion_scroll)
        .bg(theme.menu_surface)
        .border(rems(0.0625))
        .border_color(theme.menu_border)
        .rounded_sm()
        .shadow(crate::ui::style::rem_shadow(false))
        .occlude()
        .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
        .children(rows)
        .child(
            canvas(
                |bounds, _, _| bounds,
                move |_, bounds, window, cx| {
                    let bounds = crate::ui::components::popup::owner_bounds(window, cx, bounds);
                    measure.update(cx, |input, _| input.completion_bounds = Some(bounds));
                },
            )
            .absolute()
            .size_full(),
        );
    menu.into_any_element()
}

#[cfg(target_os = "windows")]
pub(super) fn surface(source: Entity<TextInput>) -> impl IntoElement {
    canvas(
        move |bounds, window, cx| {
            let bounds = crate::ui::components::popup::owner_bounds(window, cx, bounds);
            source.update(cx, |input, _| input.completion_anchor = Some(bounds));
            crate::ui::components::popup::show_passive(
                format!("path-completions-{:?}", source.entity_id()),
                crate::ui::components::popup::PopupAnchor::element(bounds),
                source.clone(),
                window,
                cx,
                |input| !input.completions.is_empty() && input.completion_anchor.is_some(),
                |input, window, cx| render(input, window, cx),
                |input, _, cx| {
                    input.completions.clear();
                    input.completion_anchor = None;
                    input.completion_bounds = None;
                    input.completion_epoch += 1;
                    input.completion_task = None;
                    cx.notify();
                },
            );
        },
        |_, _, _, _| {},
    )
    .absolute()
    .size_full()
}
