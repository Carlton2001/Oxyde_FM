use super::TextInput;
use crate::ui::theme::ThemeContext;
use gpui::{prelude::*, *};
pub(super) mod scroll;

pub(super) fn render(
    view: &TextInput,
    window: &mut Window,
    cx: &mut Context<TextInput>,
) -> AnyElement {
    let theme = *cx.theme();
    let content_height = window.rem_size() * (1.75 * view.completions.len() as f32);
    let viewport_height = content_height.min(window.rem_size() * 14.0).min(
        (crate::ui::components::popup::available_size(window, cx).height
            - window.rem_size() * 0.75)
            .max(px(1.0)),
    );
    let overflowing = content_height > viewport_height;
    let source = cx.entity();
    let rows = uniform_list(
        "path-completion-scroll",
        view.completions.len(),
        move |range, _, cx| {
            let input = source.read(cx);
            let selected_index = input.completion_index;
            let paths = range
                .filter_map(|index| {
                    input
                        .completions
                        .get(index)
                        .map(|path| (index, path.clone()))
                })
                .collect::<Vec<_>>();
            paths
                .into_iter()
                .map(|(index, path)| {
                    let selected = index == selected_index;
                    let handle = source.clone();
                    crate::ui::components::select_menu::row(
                        div().id(("path-completion", index)),
                        theme,
                        selected,
                    )
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
                .collect::<Vec<_>>()
        },
    )
    .size_full()
    .track_scroll(&view.completion_scroll);
    let measure = cx.entity();
    let refresh = cx.entity();
    let menu = crate::ui::components::select_menu::surface(theme)
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
        .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
        .child(
            div()
                .relative()
                .h(viewport_height)
                .flex_none()
                .child(
                    div()
                        .size_full()
                        .when(overflowing, |list| {
                            list.pr(crate::ui::components::select_menu::scrollbar_content_inset())
                        })
                        .on_scroll_wheel(move |_, _, cx| refresh.update(cx, |_, cx| cx.notify()))
                        .child(rows),
                )
                .when(overflowing, |body| {
                    body.child(scroll::render(view, viewport_height, content_height, cx))
                }),
        )
        .child(
            canvas(
                |bounds, _, _| bounds,
                move |_, bounds, window, cx| {
                    let bounds = crate::ui::components::popup::owner_bounds(window, cx, bounds);
                    measure.update(cx, |input, _| input.completion_bounds = Some(bounds));
                },
            )
            .absolute()
            .inset_0()
            .size_full(),
        );
    menu.into_any_element()
}

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
                    input.completion_pointer = Default::default();
                    cx.notify();
                },
            );
        },
        |_, _, _, _| {},
    )
    .absolute()
    .inset_0()
    .size_full()
}
