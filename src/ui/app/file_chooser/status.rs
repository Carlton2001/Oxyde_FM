//! The status bar under the entries and what an empty folder says, copies of the main
//! window's (`file_list/details.rs`, `grid.rs`, `presentation.rs`): folders and files
//! shown, the selection's summary, and the switch between details and grid. Choosing
//! has no size bars or grouping, so their buttons are left out.
use super::{FileChooserWindow, Listing};
use crate::ui::components::tooltip::{NativeTooltipExt, Tooltip};
use crate::ui::format::{format_number, format_size};
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::theme::OxydeTheme;
use gpui::prelude::*;
use gpui::*;

impl FileChooserWindow {
    /// What the list says instead of entries, and whether access was denied.
    pub(super) fn empty_message(&self, cx: &App) -> Option<(String, bool)> {
        if !self.shown.is_empty() {
            return None;
        }
        let i18n = cx.i18n();
        Some(match self.listing {
            _ if !self.entries.is_empty() => (i18n.text(Message::EmptyFiltered).to_string(), false),
            Listing::Loading => (i18n.text(Message::FolderLoading).to_string(), false),
            Listing::Ready => (i18n.text(Message::FolderEmpty).to_string(), false),
            Listing::Failed(message) => (i18n.text(message).to_string(), message == Message::PathAccessDenied),
        })
    }

    fn view_mode_switch(&self, theme: OxydeTheme, cx: &mut Context<Self>) -> impl IntoElement {
        let mut switch = crate::ui::elements::segmented::segmented("file-chooser-view-mode", None, theme).p(rems(0.0625));
        for (grid, name, icon, tooltip) in [
            (false, "file-chooser-view-details", "icons/list.svg", Message::ViewDetails),
            (true, "file-chooser-view-grid", "icons/layout-grid.svg", Message::ViewGrid),
        ] {
            let selected = grid == self.options.grid;
            switch = switch.child(
                crate::ui::elements::segmented::segment(name, selected, true, theme)
                    .h(rems(1.25))
                    .w(rems(1.5))
                    .px_0()
                    .fixed_tooltip(Tooltip::text(cx.i18n().text(tooltip)))
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.set_grid(grid, cx);
                    }))
                    .child(svg().path(icon).size(rems(0.875)).text_color(if selected { theme.accent } else { theme.text })),
            );
        }
        switch
    }

    pub(super) fn render_status_bar(&self, theme: OxydeTheme, cx: &mut Context<Self>) -> impl IntoElement {
        let language = cx.i18n().language();
        let count_label = |count: usize, message| {
            let mut args = FluentArgs::new();
            args.set("count", count);
            args.set("number", format_number(count as u64, language));
            cx.i18n().text_with_args(message, &args)
        };
        let folders_label = count_label(self.folders, Message::StatusFolders);
        let files_label = count_label(self.shown.len().saturating_sub(self.folders), Message::StatusFiles);
        let selection_summary = self.selected.and_then(|position| self.shown_entry(position)).map(|entry| {
            let (folders, files) = if entry.is_dir { (1, 0) } else { (0, 1) };
            let size = if !entry.is_dir || entry.is_calculated { entry.size } else { 0 };
            let mut args = FluentArgs::new();
            args.set("count", 1);
            args.set("number", format_number(1, language));
            args.set("folder_count", folders);
            args.set("file_count", files);
            args.set("folders", format_number(folders as u64, language));
            args.set("files", format_number(files as u64, language));
            args.set("size", format_size(size, false, false, language));
            cx.i18n().text_with_args(if entry.is_dir { Message::SelectedFolders } else { Message::SelectedFiles }, &args)
        });
        div()
            .occlude()
            .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .absolute()
            .left_0()
            .right_0()
            .bottom_0()
            .h(crate::ui::style::STATUS_BAR_HEIGHT)
            .min_w_0()
            .flex_none()
            .overflow_hidden()
            .flex()
            .items_center()
            .gap_3()
            .px_3()
            .pr(rems(0.5))
            .py(rems(0.25))
            .bg(theme.header)
            .border_t(rems(0.0625))
            .border_color(theme.border)
            .text_size(rems(0.75))
            .text_color(theme.text)
            .font_weight(FontWeight::MEDIUM)
            .child(folders_label)
            .child(files_label)
            .when_some(selection_summary, |bar, summary| {
                bar.child("•").child(
                    div()
                        .id("file-chooser-selection-summary")
                        .min_w_0()
                        .text_ellipsis()
                        .overflow_hidden()
                        .text_color(theme.accent)
                        .native_tooltip(Tooltip::text(summary.clone()))
                        .child(summary),
                )
            })
            .child(div().flex_1())
            .child(self.view_mode_switch(theme, cx))
    }
}
