use super::*;

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focus_handle = self.focus_handle.clone();
        let is_focused = focus_handle.is_focused(window);
        let theme = *cx.theme();
        if !is_focused || self.completions.is_empty() {
            self.completion_anchor = None;
        }

        div()
            .relative()
            .flex()
            .items_center()
            .key_context("TextInput")
            .track_focus(&focus_handle)
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::enter))
            .on_action(cx.listener(Self::escape))
            .on_action(cx.listener(Self::completion_up))
            .on_action(cx.listener(Self::completion_down))
            .on_action(cx.listener(Self::complete_path))
            .on_action(cx.listener(Self::move_left))
            .on_action(cx.listener(Self::move_right))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::on_right_mouse_down))
            .when(self.options.submit_on_outside, |div| {
                div.on_mouse_down_out(cx.listener(Self::submit_on_outside_click))
            })
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .map(|input| field::input_surface(input, self.options.appearance, is_focused, &theme))
            .when(self.options.clearable && !self.content.is_empty(), |s| {
                s.pr(rems(0.125))
            })
            .when(self.path_submit, |s| s.pr(rems(0.0625)))
            .w_full()
            .when(is_focused && !self.completions.is_empty(), |input| {
                if crate::ui::components::popup::supported(window, cx) {
                    return input.child(completion_menu::surface(cx.entity()));
                }
                let menu = completion_menu::render(self, window, cx);
                input.child(deferred(
                    div()
                        .absolute()
                        .top(relative(1.0))
                        .left_0()
                        .w_full()
                        .child(menu),
                ))
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .child(TextInputElement { input: cx.entity() }),
            )
            .when(self.path_submit, |input| {
                input.child(
                    div()
                        .id("path-submit")
                        .size_6()
                        .ml_2()
                        .flex_none()
                        .flex()
                        .occlude()
                        .cursor(CursorStyle::Arrow)
                        .items_center()
                        .justify_center()
                        .rounded_sm()
                        .hover(|style| style.bg(theme.control_hover))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, window, cx| {
                                cx.stop_propagation();
                                (this.on_enter)(window, cx);
                            }),
                        )
                        .child(
                            svg()
                                .path("icons/arrow-right.svg")
                                .size_4()
                                .text_color(theme.text),
                        ),
                )
            })
            .when(
                self.options.clearable && !self.content.is_empty(),
                |s: Div| {
                    let h_clear = cx.entity().clone();
                    s.child(
                        div().flex().items_center().child(
                            div()
                                .p_1()
                                .rounded_md()
                                .cursor(CursorStyle::Arrow)
                                .hover(|s| s.bg(theme.control_hover))
                                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                    h_clear.update(cx, |this, cx| {
                                        this.content.clear();
                                        this.selected_range = 0..0;
                                        (this.on_change)("", window, cx);
                                        cx.notify();
                                    });
                                })
                                .child(
                                    svg()
                                        .path("icons/x.svg")
                                        .size_3()
                                        .text_color(theme.text),
                                ),
                        ),
                    )
                },
            )
    }
}
