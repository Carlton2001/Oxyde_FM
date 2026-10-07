use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ButtonVariant {
    Ghost,
    Subtle,
}

#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: Option<SharedString>,
    label_size: Option<f32>,
    variant: ButtonVariant,
    on_click: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
}

impl Button {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            label: None,
            label_size: None,
            variant: ButtonVariant::Ghost,
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

    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
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

        let (bg, border_color, text_color) = match self.variant {
            ButtonVariant::Subtle => (theme.surface, theme.border, theme.text),
            ButtonVariant::Ghost => (
                hsla(0.0, 0.0, 0.0, 0.0),
                hsla(0.0, 0.0, 0.0, 0.0),
                theme.text_muted,
            ),
        };

        div()
            .id(self.id)
            .flex()
            .items_center()
            .justify_center()
            .px_3()
            .py_1()
            .h(rems(1.5))
            .rounded_md()
            .bg(bg)
            .border(rems(0.0625))
            .border_color(border_color)
            .hover(|style| style.bg(theme.hover).border_color(theme.accent.opacity(0.5)))
            .active(|style| style.bg(theme.selected).border_color(theme.accent))
            .when_some(self.on_click, |container, on_click| {
                container
                    .on_mouse_down(MouseButton::Left, move |_, window, cx| on_click(window, cx))
            })
            .children(self.label.map(|label| {
                div()
                    .ml_2()
                    .when_some(self.label_size, |label, size| label.text_size(rems(size / 16.0)))
                    .when(self.label_size.is_none(), |label| label.text_sm())
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(text_color)
                    .child(label)
            }))
    }
}
