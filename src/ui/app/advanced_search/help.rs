use super::AdvancedSearchWindow;
use crate::ui::{
    components::scrollbar::{render_scrollbar_with_pointer, ScrollbarAxis},
    i18n::{I18nContext, Message},
    theme::ThemeContext,
};
use gpui::prelude::*;
use gpui::*;

impl AdvancedSearchWindow {
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
        let thumb =
            (viewport * viewport / content).max(crate::ui::scale::rem_pixels(rems(2.5)).as_f32());
        let travel = (viewport - thumb).max(1.0);
        let offset = ((position.y.as_f32() - origin.y - self.grab_offset) / travel).clamp(0.0, 1.0)
            * (content - viewport);
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
        let patterns = [
            ".", "\\d", "\\w", "^", "$", "+", "?", "|", "[abc]", "[^abc]", "()", "{n}",
        ];
        let labels = [
            Message::AdvancedRegexAnyChar,
            Message::AdvancedRegexDigit,
            Message::AdvancedRegexWordChar,
            Message::AdvancedRegexStartLine,
            Message::AdvancedRegexEndLine,
            Message::AdvancedRegexOneOrMore,
            Message::AdvancedRegexOptional,
            Message::AdvancedRegexEitherOr,
            Message::AdvancedRegexRange,
            Message::AdvancedRegexNegatedSet,
            Message::AdvancedRegexGroup,
            Message::AdvancedRegexCountExact,
        ];
        let descriptions = [
            Message::AdvancedRegexAnyCharDesc,
            Message::AdvancedRegexDigitDesc,
            Message::AdvancedRegexWordCharDesc,
            Message::AdvancedRegexStartLineDesc,
            Message::AdvancedRegexEndLineDesc,
            Message::AdvancedRegexOneOrMoreDesc,
            Message::AdvancedRegexOptionalDesc,
            Message::AdvancedRegexEitherOrDesc,
            Message::AdvancedRegexRangeDesc,
            Message::AdvancedRegexNegatedSetDesc,
            Message::AdvancedRegexGroupDesc,
            Message::AdvancedRegexCountExactDesc,
        ];
        body = body.child(
            div()
                .flex()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(rems(0.5))
                        .child(
                            svg()
                                .path("icons/code.svg")
                                .size(rems(0.875))
                                .text_color(theme.text_muted),
                        )
                        .child(cx.i18n().text(Message::AdvancedRegexHelpTitle)),
                )
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
            .on_scroll_wheel(move |_, _, cx| refresh.update(cx, |_, cx| cx.notify()));
        for i in 0..patterns.len() {
            rows = rows.child(
                div()
                    .flex()
                    .flex_none()
                    .w_full()
                    .min_w_0()
                    .items_center()
                    .gap(rems(0.75))
                    .py(rems(0.625))
                    .border_b(rems(0.0625))
                    .border_color(theme.border)
                    .child(
                        div()
                            .w(rems(4.0))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_md()
                            .bg(theme.header)
                            .border(rems(0.0625))
                            .border_color(theme.border)
                            .py(rems(0.25))
                            .text_color(theme.accent)
                            .child(patterns[i]),
                    )
                    .child(
                        div()
                            .w(rems(10.0))
                            .flex_none()
                            .child(cx.i18n().text(labels[i])),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .whitespace_normal()
                            .text_color(theme.text_muted)
                            .child(cx.i18n().text(descriptions[i])),
                    ),
            );
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
