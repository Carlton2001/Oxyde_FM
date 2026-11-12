use super::DuplicateWindow;
use crate::ui::components::tool_window::title_button;
use crate::ui::i18n::{I18nContext, Message};
use gpui::prelude::*;
use gpui::*;

impl DuplicateWindow {
    pub(super) fn render_title_actions(&self, cx: &Context<Self>) -> Div {
        let collapse = cx.entity();
        let top = cx.entity();
        let can_collapse = self.result_rows.has_expanded;
        div()
            .h_full()
            .flex()
            .flex_none()
            .child(title_button(
                "duplicates-collapse-all",
                if can_collapse {
                    "icons/chevrons-up.svg"
                } else {
                    "icons/chevrons-down.svg"
                },
                cx.i18n().text(if can_collapse {
                    Message::SidebarCollapseAll
                } else {
                    Message::ExpandAll
                }),
                !self.filtered.is_empty(),
                cx,
                move |_, _, cx| {
                    collapse.update(cx, |this, cx| {
                        if can_collapse {
                            this.collapsed.extend(0..this.groups.len());
                        } else {
                            this.collapsed.clear();
                        }
                        this.prepare_results(false, cx);
                        cx.notify();
                    });
                },
            ))
            .child(title_button(
                "duplicates-top",
                "icons/arrow-up-to-line.svg",
                cx.i18n().text(Message::SidebarScrollTop),
                crate::ui::components::scrollbar::scrolled_from_top(&self.results_scroll),
                cx,
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
