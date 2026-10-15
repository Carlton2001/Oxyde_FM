mod style;
mod toolbar;
pub(crate) use toolbar::toolbar_button_with_tooltip;
pub use style::{button_style, ButtonVariant};

use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;

#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: Option<SharedString>,
    label_size: Option<f32>,
    icon: Option<&'static str>,
    on_click: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
}

impl Button {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            label: None,
            label_size: None,
            icon: None,
            on_click: None,
        }
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn label_size(mut self, size: f32) -> Self {
        self.label_size = Some(size);
        self
    }

    pub fn icon(mut self, path: &'static str) -> Self {
        self.icon = Some(path);
        self
    }

    pub fn on_click(mut self, on_click: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Box::new(on_click));
        self
    }
}

impl RenderOnce for Button {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();

        button_style(div().id(self.id), *theme, ButtonVariant::Secondary, true)
            .flex()
            .items_center()
            .justify_center()
            .gap(rems(0.5))
            .px_3()
            .py_1()
            .h(rems(1.5))
            .rounded_md()
            .border(rems(0.0625))
            .border_color(theme.border)
            .when_some(self.on_click, |container, on_click| {
                container
                    .on_mouse_down(MouseButton::Left, move |_, window, cx| on_click(window, cx))
            })
            .children(self.icon.map(|path| {
                svg()
                    .path(path)
                    .size(rems(0.875))
                    .flex_none()
                    .text_color(theme.text)
            }))
            .children(self.label.map(|label| {
                div()
                    .when_some(self.label_size, |label, size| {
                        label.text_size(rems(size / 16.0))
                    })
                    .when(self.label_size.is_none(), |label| label.text_sm())
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.text)
                    .child(label)
            }))
    }
}
