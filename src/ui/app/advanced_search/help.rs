use super::AdvancedSearchWindow;
use crate::ui::components::tooltip::NativeTooltipExt;
use crate::ui::{
    components::scrollbar::{render_scrollbar_with_pointer, ScrollbarAxis},
    i18n::{I18nContext, Message},
    theme::ThemeContext,
};
use gpui::prelude::*;
use gpui::*;

/// Name-field syntax without a regular expression: the pattern, its meaning, an example.
const WILDCARDS: [(&str, Message, &str); 2] = [
    ("*", Message::AdvancedHelpAnySequence, "*.pdf"),
    ("?", Message::AdvancedHelpOneChar, "photo?.jpg"),
];

/// Regular expressions match anywhere in the name unless anchored.
const REGEX: [(&str, Message, &str); 11] = [
    (".", Message::AdvancedHelpOneChar, "t.t"),
    (".*", Message::AdvancedHelpAnySequence, "IMG.*2024"),
    (r"\.", Message::AdvancedHelpLiteralDot, r"\.txt$"),
    (r"\d", Message::AdvancedHelpDigit, r"IMG_\d"),
    ("+", Message::AdvancedHelpOneOrMore, r"IMG_\d+"),
    ("?", Message::AdvancedHelpOptional, r"\.jpe?g$"),
    ("^  $", Message::AdvancedHelpStartEnd, "^IMG"),
    ("a|b", Message::AdvancedHelpEitherOr, r"\.(jpg|png)$"),
    ("[abc]", Message::AdvancedHelpOneOf, "v[0-9]"),
    ("[^abc]", Message::AdvancedHelpNoneOf, "^[^.]+$"),
    ("x{n}", Message::AdvancedHelpCount, r"\d{4}"),
];

fn help_heading(title: SharedString, cx: &App) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_between()
        .pb(rems(0.25))
        .mb(rems(0.25))
        .border_b(rems(0.0625))
        .border_color(cx.theme().border)
        .text_color(cx.theme().text_muted)
        .child(title)
}

/// A pattern, its meaning, and an example that fills the name field when clicked.
fn help_row(
    index: usize,
    pattern: &'static str,
    label: Message,
    example: &'static str,
    regex: bool,
    handle: Entity<AdvancedSearchWindow>,
    cx: &App,
) -> Div {
    let theme = *cx.theme();
    div()
        .flex()
        .flex_none()
        .w_full()
        .min_w_0()
        .items_center()
        .gap(rems(0.75))
        .py(rems(0.1875))
        .child(
            div()
                .w(rems(3.5))
                .flex_none()
                .font_family("JetBrains Mono")
                .text_color(theme.accent)
                .child(pattern),
        )
        .child(div().flex_1().min_w_0().truncate().child(cx.i18n().text(label)))
        .child(
            div()
                .id(("advanced-help-example", index))
                .flex_none()
                .px(rems(0.5))
                .py(rems(0.125))
                .rounded_md()
                .bg(theme.header)
                .border(rems(0.0625))
                .border_color(theme.border)
                .font_family("JetBrains Mono")
                .hover(move |style| style.bg(theme.control_hover))
                .child(example)
                .fixed_tooltip(crate::ui::components::tooltip::Tooltip::text(
                    cx.i18n().text(Message::AdvancedHelpUseExample),
                ))
                .on_click(move |_, window, cx| {
                    handle.update(cx, |this, cx| this.use_help_example(example, regex, window, cx));
                }),
        )
}

impl AdvancedSearchWindow {
    /// Puts the example in the name field, with the matching syntax, and returns to it.
    fn use_help_example(&mut self, example: &str, regex: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(d) = &mut self.advanced_search else {
            return;
        };
        d.flags[0] = regex;
        d.help = false;
        d.error = false;
        d.values[0] = example.into();
        let input = d.inputs[0].clone();
        input.update(cx, |input, cx| input.set_content(example.to_string(), cx));
        let focus = input.read(cx).focus_handle.clone();
        window.focus(&focus, cx);
        cx.notify();
    }

    fn drag_advanced_help(&mut self, position: Point<Pixels>, cx: &mut App) {
        let Some(d) = &mut self.advanced_search else {
            return;
        };
        let Some((track, viewport, content)) = &d.help_drag else {
            return;
        };
        let Some(origin) = self.scrollbar_origins.get(track) else {
            return;
        };
        let offset = crate::ui::components::scrollbar::drag_scrolled(
            *viewport, *content, *viewport, position.y.as_f32() - origin.y, self.grab_offset);
        d.help_scroll.set_offset(point(px(0.0), px(-offset)));
        if let Some(handle) = self.handle.upgrade() {
            cx.notify(handle.entity_id());
        }
    }

    pub(super) fn render_advanced_help(
        &self,
        mut body: Div,
        handle: Entity<Self>,
        cx: &mut Context<Self>,
    ) -> Div {
        let d = self.advanced_search.as_ref().unwrap();
        let theme = *cx.theme();
        let refresh = handle.clone();
        let mut rows = div()
            .id("advanced-help-scroll")
            .h_full()
            .w_full()
            .pr(rems(0.75))
            .track_scroll(&d.help_scroll)
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .on_scroll_wheel(move |_, _, cx| refresh.update(cx, |_, cx| cx.notify()))
            .child(help_heading(cx.i18n().text(Message::AdvancedWildcardsTitle), cx));
        for (index, &(pattern, label, example)) in WILDCARDS.iter().enumerate() {
            rows = rows.child(help_row(index, pattern, label, example, false, handle.clone(), cx));
        }
        rows = rows.child(
            help_heading(cx.i18n().text(Message::AdvancedRegexHelpTitle), cx)
                .mt(rems(0.75))
                .child(
                    div()
                        .id("regex-link")
                        .flex()
                        .items_center()
                        .gap(rems(0.375))
                        .text_color(theme.accent)
                        .child("regex101.com")
                        .child(
                            svg()
                                .path("icons/external-link.svg")
                                .size(rems(0.625))
                                .text_color(theme.accent),
                        )
                        .on_click(|_, _, cx| {
                            cx.background_spawn(async {
                                let _ = oxyde_lib::platform::platform()
                                    .open_item("https://regex101.com");
                            })
                            .detach();
                        }),
                ),
        );
        for (index, &(pattern, label, example)) in REGEX.iter().enumerate() {
            rows = rows.child(help_row(WILDCARDS.len() + index, pattern, label, example, true, handle.clone(), cx));
        }
        let measure = handle.clone();
        let mouse = handle.clone();
        let viewport = d.help_metrics.0;
        let content = d.help_metrics.1;
        body = body.child(
            div()
                .relative()
                .flex_1()
                .min_h_0()
                .w_full()
                .min_w_0()
                .child(rows)
                .child(render_scrollbar_with_pointer(
                    theme,
                    handle.clone(),
                    ScrollbarAxis::Vertical,
                    viewport,
                    content,
                    d.help_scroll.offset().y.as_f32().abs(),
                    d.help_drag.is_some(),
                    "advanced-help".into(),
                    None,
                    d.help_scroll.clone(),
                    |this, track, viewport, content, position, cx| {
                        if let Some(d) = &mut this.advanced_search {
                            d.help_drag = Some((track, viewport, content));
                        }
                        this.drag_advanced_help(position, cx);
                    },
                ))
                .child(
                    canvas(
                        |_, _, _| (),
                        move |_, _, window, cx| {
                            measure.update(cx, |this, cx| {
                                if let Some(d) = &mut this.advanced_search {
                                    let viewport = d.help_scroll.bounds().size.height.as_f32();
                                    let metrics = (
                                        viewport,
                                        viewport + d.help_scroll.max_offset().y.as_f32(),
                                    );
                                    if d.help_metrics != metrics {
                                        d.help_metrics = metrics;
                                        cx.notify();
                                    }
                                }
                            });
                            let movement = mouse.clone();
                            window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                                if phase == DispatchPhase::Capture {
                                    movement.update(cx, |this, cx| {
                                        if this
                                            .advanced_search
                                            .as_ref()
                                            .is_some_and(|d| d.help_drag.is_some())
                                        {
                                            if event.dragging() {
                                                this.drag_advanced_help(event.position, cx);
                                            } else if let Some(d) = &mut this.advanced_search {
                                                d.help_drag = None;
                                                cx.notify();
                                            }
                                        }
                                    });
                                }
                            });
                            let release = mouse.clone();
                            window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                                if phase == DispatchPhase::Capture
                                    && event.button == MouseButton::Left
                                {
                                    release.update(cx, |this, cx| {
                                        if let Some(d) = &mut this.advanced_search {
                                            if d.help_drag.take().is_some() {
                                                cx.notify();
                                            }
                                        }
                                    });
                                }
                            });
                        },
                    )
                    .absolute()
                    .size_0(),
                ),
        );
        body
    }
}
