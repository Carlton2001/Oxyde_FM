use crate::ui::scale::{rem_pixels};
use gpui::*;

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum IconSize {
    XSmall,
    Small,
    Medium,
}

#[allow(dead_code)]
impl IconSize {
    pub fn px(&self) -> Pixels {
        match self {
            Self::XSmall => rem_pixels(rems(0.75)),
            Self::Small => rem_pixels(rems(1.0)),
            Self::Medium => rem_pixels(rems(1.25)),
        }
    }
}

#[allow(dead_code)]
#[derive(IntoElement)]
pub struct Icon {
    path: SharedString,
    size: IconSize,
    color: Option<Hsla>,
}

#[allow(dead_code)]
impl Icon {
    pub fn new(path: impl Into<SharedString>) -> Self {
        Self {
            path: path.into(),
            size: IconSize::Small,
            color: None,
        }
    }

    pub fn size(mut self, size: IconSize) -> Self {
        self.size = size;
        self
    }

    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }
}

impl RenderOnce for Icon {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let color = self
            .color
            .unwrap_or_else(|| crate::ui::theme::OxydeTheme::global(cx).text_muted);

        svg().path(self.path).size(self.size.px()).text_color(color)
    }
}
