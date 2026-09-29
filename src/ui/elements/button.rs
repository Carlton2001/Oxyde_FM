use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ButtonVariant {
    Ghost,
    Primary,
    Subtle,
}

#[allow(dead_code)]
#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: Option<SharedString>,
    label_size: Option<f32>,
    icon: Option<SharedString>,
    variant: ButtonVariant,
    on_click: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
    selected: bool,
    tooltip: Option<SharedString>,
}

#[allow(dead_code)]
impl Button {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            label: None,
            label_size: None,
            icon: None,
            variant: ButtonVariant::Ghost,
            on_click: None,
            selected: false,
            tooltip: None,
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

    pub fn icon(mut self, icon: impl Into<SharedString>) -> Self {
        self.icon = Some(icon.into());
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

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }
}

impl RenderOnce for Button {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();

        // Define colors based on variant and selection
        let (bg, border_color, text_color) = match (self.variant, self.selected) {
            (_, true) => (theme.selected, theme.accent, theme.text),
            (ButtonVariant::Primary, _) => (theme.accent, theme.accent, theme.text_on_accent),
            (ButtonVariant::Subtle, _) => (theme.surface, theme.border, theme.text),
            (ButtonVariant::Ghost, _) => (
                hsla(0.0, 0.0, 0.0, 0.0),
                hsla(0.0, 0.0, 0.0, 0.0),
                theme.text_muted,
            ),
        };
        let filled = self.variant == ButtonVariant::Primary && !self.selected;

        div()
            .id(self.id)
            .flex()
            .items_center()
            .justify_center()
            .px_3()
            .py_1()
            .h(px(24.0))
            .rounded_md()
            .bg(bg)
            .border_1()
            .border_color(border_color)
            .hover(|style| {
                if filled {
                    style.bg(theme.accent).border_color(theme.accent)
                } else {
                    style
                        .bg(theme.hover)
                        .border_color(theme.accent.opacity(0.5))
                }
            })
            .active(|style| {
                if filled {
                    style.bg(theme.accent).border_color(theme.accent)
                } else {
                    style.bg(theme.selected).border_color(theme.accent)
                }
            })
            .when_some(self.on_click, |container, on_click| {
                container
                    .on_mouse_down(MouseButton::Left, move |_, window, cx| on_click(window, cx))
            })
            .children(self.icon.map(|icon_path| {
                svg()
                    .path(icon_path)
                    .size_3p5()
                    .text_color(if self.selected {
                        theme.text
                    } else {
                        theme.text_muted
                    })
            }))
            .children(self.label.map(|label| {
                div()
                    .ml_2()
                    .when_some(self.label_size, |label, size| label.text_size(px(size)))
                    .when(self.label_size.is_none(), |label| label.text_sm())
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(text_color)
                    .child(label)
            }))
    }
}
