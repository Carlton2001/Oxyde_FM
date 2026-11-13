//! The body shared by confirmation dialogs: an icon in a tinted disc, the question,
//! what it concerns, and a warning when the action cannot be undone. Their buttons are
//! `dialog::button`, the action taking `Tone::variant`.
use crate::ui::elements::button::ButtonVariant;
use crate::ui::theme::OxydeTheme;
use gpui::prelude::FluentBuilder;
use gpui::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tone {
    Neutral,
    Danger,
}

impl Tone {
    /// The action asked for, red when it destroys.
    pub(crate) fn variant(self) -> ButtonVariant {
        match self {
            Self::Neutral => ButtonVariant::Primary,
            Self::Danger => ButtonVariant::Danger,
        }
    }
}

/// An empty question leaves the details alone, for a notice.
pub(crate) fn body(
    icon: &'static str,
    tone: Tone,
    question: SharedString,
    details: Vec<AnyElement>,
    warning: Option<SharedString>,
    theme: OxydeTheme,
) -> Div {
    let color = match tone {
        Tone::Neutral => theme.accent,
        Tone::Danger => theme.danger_background,
    };
    div()
        .flex()
        .flex_col()
        .gap(rems(0.875))
        .child(
            div()
                .flex()
                .items_start()
                .gap(rems(0.875))
                .child(
                    div()
                        .flex_none()
                        .size(rems(2.5))
                        .rounded_full()
                        .bg(color.opacity(0.14))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(svg().path(icon).size(rems(1.25)).text_color(color)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .pt(rems(0.125))
                        .flex()
                        .flex_col()
                        .gap(rems(0.25))
                        .when(!question.is_empty(), |text| {
                            text.child(div().text_size(rems(0.875)).font_weight(FontWeight::MEDIUM).child(question))
                        })
                        .children(details),
                ),
        )
        .when_some(warning, |body, warning| {
            body.child(
                div()
                    .flex()
                    .items_center()
                    .gap(rems(0.5))
                    .px(rems(0.75))
                    .py(rems(0.5))
                    .rounded_md()
                    .border(rems(0.0625))
                    .border_color(theme.warning_border)
                    .bg(theme.warning_background)
                    .text_color(theme.warning_text)
                    .child(svg().path("icons/triangle-alert.svg").size(rems(0.875)).flex_none().text_color(theme.warning_text))
                    .child(div().flex_1().min_w_0().child(warning)),
            )
        })
}

/// A secondary line under the question.
pub(crate) fn detail(text: impl Into<SharedString>, theme: OxydeTheme) -> AnyElement {
    div().min_w_0().text_color(theme.text_muted).child(text.into()).into_any_element()
}
