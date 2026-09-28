use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;

/// Shared presentation for centered dialogs. The caller owns business state,
/// action callbacks, initial focus, keyboard bindings and placement in the
/// render tree (including `deferred` and ordering relative to context menus).
///
/// The backdrop always blocks mouse input. Dismissal is opt-in and only reacts
/// to the left button. No Escape/Enter binding or focus change is implicit.
#[derive(IntoElement)]
pub struct Modal {
    id: ElementId,
    title: SharedString,
    header_action: Option<AnyElement>,
    body: AnyElement,
    actions: Vec<AnyElement>,
    width: Pixels,
    body_gap: Pixels,
    on_backdrop_dismiss: Option<Box<dyn Fn(&mut Window, &mut App)>>,
}

impl Modal {
    pub fn new(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        body: impl IntoElement,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            header_action: None,
            body: body.into_any_element(),
            actions: Vec::new(),
            width: px(440.0),
            body_gap: px(12.0),
            on_backdrop_dismiss: None,
        }
    }

    pub fn width(mut self, width: Pixels) -> Self {
        self.width = width;
        self
    }

    pub fn header_action(mut self, action: impl IntoElement) -> Self {
        self.header_action = Some(action.into_any_element());
        self
    }

    pub fn body_gap(mut self, gap: Pixels) -> Self {
        self.body_gap = gap;
        self
    }

    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.actions.push(action.into_any_element());
        self
    }

    pub fn on_backdrop_dismiss(
        mut self,
        callback: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_backdrop_dismiss = Some(Box::new(callback));
        self
    }
}

impl RenderOnce for Modal {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme.modal_backdrop)
            .occlude()
            .on_any_mouse_down(move |event, window, cx| {
                cx.stop_propagation();
                if event.button == MouseButton::Left {
                    if let Some(dismiss) = &self.on_backdrop_dismiss {
                        dismiss(window, cx);
                    }
                }
            })
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .id(self.id)
                    .w(self.width)
                    .max_w(window.viewport_size().width - px(32.0))
                    .max_h(window.viewport_size().height - px(32.0))
                    .flex()
                    .flex_col()
                    .p(px(20.0))
                    .rounded_lg()
                    .bg(theme.surface)
                    .text_color(theme.text)
                    .border_1()
                    .border_color(theme.border)
                    .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
                    .child(
                        div()
                            .flex()
                            .flex_none()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(px(16.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(self.title),
                            )
                            .children(self.header_action),
                    )
                    .child(
                        div()
                            .id("modal-body")
                            .min_h_0()
                            .overflow_y_scroll()
                            .mt(self.body_gap)
                            .child(self.body),
                    )
                    .when(!self.actions.is_empty(), |panel| {
                        panel.child(
                            div()
                                .flex_none()
                                .mt(px(20.0))
                                .flex()
                                .justify_end()
                                .gap(px(8.0))
                                .children(self.actions),
                        )
                    }),
            )
    }
}
