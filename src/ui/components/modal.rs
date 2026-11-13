use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;

/// Standard compact label/value row for internal popup forms.
pub fn form_row(label: SharedString, value: impl IntoElement, cx: &App) -> impl IntoElement {
    form_row_with_label_width(label, value, rems(5.5), cx)
}

/// Forms with longer labels can reserve their own label column.
pub fn form_row_with_label_width(
    label: SharedString,
    value: impl IntoElement,
    width: Rems,
    cx: &App,
) -> impl IntoElement {
    responsive_form_row(label, value, width, false, cx)
}

/// The calling view chooses when its label and value should stack.
pub(crate) fn responsive_form_row(
    label: SharedString,
    value: impl IntoElement,
    width: Rems,
    stacked: bool,
    cx: &App,
) -> Div {
    div()
        .flex()
        .items_center()
        .gap(rems(0.75))
        .min_w_0()
        .when(stacked, |row| row.flex_col().items_stretch().gap(rems(0.375)))
        .when(!stacked || !label.is_empty(), |row| row.child(
            div()
                .w(width)
                .when(stacked, |label| label.w_full())
                .flex_none()
                .text_size(rems(0.8125))
                .text_color(cx.theme().text_muted)
                .child(label),
        ))
        .child(div().min_w_0()
            .when(!stacked, |value| value.flex_1())
            .when(stacked, |value| value.w_full()).child(value))
}

/// The message of a refused field, under it and aligned to its right edge: the field
/// shakes, this says why.
pub(crate) fn field_error(message: SharedString, theme: crate::ui::theme::OxydeTheme) -> Div {
    div()
        .w_full()
        .flex()
        .items_center()
        .justify_end()
        .gap(rems(0.375))
        .text_size(rems(0.75))
        .text_color(theme.error)
        .child(svg().path("icons/circle-x.svg").size(rems(0.75)).flex_none().text_color(theme.error))
        .child(message)
}

/// The frame of a modal prompt or dialog window: the secondary windows' title bar, a
/// scrolling body and a footer of actions. The window is the prompt: the caller owns
/// business state, callbacks, initial focus and keyboard bindings.
#[derive(IntoElement)]
pub struct Modal {
    id: ElementId,
    title: SharedString,
    title_icon: Option<SharedString>,
    body: AnyElement,
    actions: Vec<AnyElement>,
    on_close: Option<Box<dyn Fn(&mut Window, &mut App)>>,
    header: Option<AnyElement>,
    body_padding: (Rems, Rems),
    fill_window: bool,
    content_width: Option<Pixels>,
}

impl Modal {
    pub(crate) fn content_width(mut self, width: Option<Pixels>) -> Self {
        self.content_width = width;
        self
    }

    /// The body takes the window's height instead of fitting the window to it.
    pub(crate) fn fill_window(mut self, fill: bool) -> Self {
        self.fill_window = fill;
        self
    }

    pub fn new(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        body: impl IntoElement,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            title_icon: None,
            body: body.into_any_element(),
            actions: Vec::new(),
            on_close: None,
            header: None,
            body_padding: (rems(1.0), rems(1.25)),
            fill_window: false,
            content_width: None,
        }
    }

    pub fn body_padding(mut self, horizontal: Rems, vertical: Rems) -> Self {
        self.body_padding = (horizontal, vertical);
        self
    }

    pub fn title_icon(mut self, path: impl Into<SharedString>) -> Self {
        self.title_icon = Some(path.into());
        self
    }

    pub fn header(mut self, header: impl IntoElement) -> Self {
        self.header = Some(header.into_any_element());
        self
    }

    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.actions.push(action.into_any_element());
        self
    }

    /// The title bar's close button.
    pub fn on_close(mut self, callback: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Box::new(callback));
        self
    }
}

impl RenderOnce for Modal {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        let height_limit = if self.fill_window { super::client_decorations::content_bounds(window).size.height } else {
            crate::ui::components::tool_window::content_height_limit(window, cx)
        };
        let fill_window = self.fill_window;
        let content_width = self.content_width;
        let on_close = self.on_close;
        let title = self.title;
        let header = self.header.unwrap_or_else(|| {
            // Same title bar as Properties and the other secondary windows.
            window.set_window_title(title.as_ref());
            super::tool_window::header(
                self.title_icon.unwrap_or_default(), title, window, false, cx,
                move |window, cx| if let Some(close) = &on_close { close(window, cx) },
            ).into_any_element()
        });
        let body = super::scroll_area::scroll_area((self.id.clone(), "body-scroll"),
            div().when(fill_window, |frame| frame.flex_1()),
            div().id("modal-body").px(self.body_padding.0).py(self.body_padding.1)
                .flex().flex_col().gap(rems(0.375))
                .when(fill_window, |body| body.flex_1()).child(self.body), false, window, cx);
        let corners = super::client_decorations::inner_corners(window);
        div().id(self.id).relative().w_full().max_h(height_limit)
            // Its background reaches all four corners: the header's rounding alone leaves it square.
            .rounded_tl(corners.top_left).rounded_tr(corners.top_right)
            .rounded_bl(corners.bottom_left).rounded_br(corners.bottom_right)
            .when(fill_window, |frame| frame.h_full())
            .flex().flex_col().bg(theme.active_background)
            .text_color(theme.text).text_size(rems(0.8125)).line_height(relative(1.25))
            .child(header)
            .child(body)
            .when(!self.actions.is_empty(), |frame| frame.child(
                super::tool_window::footer(cx).children(self.actions)))
            .child(canvas(|bounds, _, _| bounds, move |_, bounds, window, cx| {
                // The laid-out width only follows the window: chasing it, snapped to device
                // pixels, would widen the window frame after frame at fractional scales.
                if !fill_window {
                    match content_width {
                        Some(width) => crate::ui::components::tool_window::fit_content_height_at_width(
                            bounds.size.height, width, window, cx),
                        None => crate::ui::components::tool_window::fit_content_height(
                            bounds.size.height, window, cx),
                    }
                }
            }).absolute().size_full())
    }
}
