use super::*;

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focus_handle = self.focus_handle.clone();
        let is_focused = focus_handle.is_focused(window);
        let theme = *cx.theme();
        if !is_focused || self.completions.is_empty() {
            self.completion_anchor = None;
        }
        let clear = self.options.clearable && !self.content.is_empty();
        let trailing_button = clear || self.path_submit;
        let invalid = self.invalid_text.as_deref() == Some(self.content.as_str());

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
            .map(|input| field::input_surface(input, self.options.appearance, is_focused, invalid, &theme))
            // A trailing button cell replaces the right text margin and meets the frame edge.
            .when(trailing_button, |input| input.pr_0())
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
            .when(clear, |input| {
                let h_clear = cx.entity().clone();
                input.child(
                    field::field_button("input-clear", FieldEdge::Trailing, false, &theme)
                        .cursor(CursorStyle::Arrow)
                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                            cx.stop_propagation();
                            h_clear.update(cx, |this, cx| {
                                this.content.clear();
                                this.selected_range = 0..0;
                                (this.on_change)("", window, cx);
                                cx.notify();
                            });
                        })
                        .child(field::field_button_icon("icons/x.svg", false, &theme)),
                )
            })
            .when(self.path_submit, |input| {
                input.child(
                    field::field_button("path-submit", FieldEdge::Trailing, false, &theme)
                        .cursor(CursorStyle::Arrow)
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, window, cx| {
                                cx.stop_propagation();
                                (this.on_enter)(window, cx);
                            }),
                        )
                        .child(field::field_button_icon("icons/arrow-right.svg", false, &theme)),
                )
            })
    }
}
