use super::DuplicateWindow;
use crate::ui::components::tooltip::Tooltip;
use crate::ui::elements::button::toolbar_button_with_tooltip;
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;

impl DuplicateWindow {
    pub(super) fn render_title_actions(&self, cx: &Context<Self>) -> Div {
        let collapse = cx.entity();
        let top = cx.entity();
        let can_collapse = self.filtered.iter().any(|index| !self.collapsed.contains(index));
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(rems(0.25))
            .mr(rems(0.375))
            .child(toolbar_button_with_tooltip(
                "duplicates-collapse-all".into(),
                Tooltip::text(cx.i18n().text(if can_collapse {
                    Message::SidebarCollapseAll
                } else {
                    Message::ExpandAll
                })),
                Some(if can_collapse {
                    "icons/chevrons-up.svg"
                } else {
                    "icons/chevrons-down.svg"
                }),
                None,
                *cx.theme(),
                !self.filtered.is_empty(),
                move |_, _, cx| {
                    collapse.update(cx, |this, cx| {
                        if can_collapse {
                            this.collapsed.extend(0..this.groups.len());
                        } else {
                            this.collapsed.clear();
                        }
                        cx.notify();
                    });
                },
            ))
            .child(toolbar_button_with_tooltip(
                "duplicates-top".into(),
                Tooltip::text(cx.i18n().text(Message::SidebarScrollTop)),
                Some("icons/arrow-up-to-line.svg"),
                None,
                *cx.theme(),
                self.results_scroll.offset().y < px(0.0),
                move |_, _, cx| {
                    top.update(cx, |this, cx| {
                        let offset = this.results_scroll.offset();
                        this.results_scroll.set_offset(point(offset.x, px(0.0)));
                        cx.notify();
                    });
                },
            ))
    }
}
