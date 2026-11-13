//! The selected operation, right of the list: its name and state, its progress,
//! where its items come from and go, and why it failed. Its commands sit in the
//! window's footer.
pub(super) mod controls;
mod graph;
mod metrics;

use super::OperationsPanelView;
use crate::ui::format::format_number;
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::theme::{OxydeTheme, ThemeContext};
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::systems::file_ops::{FileOpType, FileOperation, OpStatus};
use std::path::{Path, PathBuf};

pub(super) fn status_color(status: &OpStatus, theme: OxydeTheme) -> Hsla {
    match status {
        OpStatus::Error(_) => theme.error,
        OpStatus::Completed => theme.success_text,
        // Amber warning sits too close to most accents: a pause dims instead.
        OpStatus::Paused | OpStatus::Cancelled => theme.text_muted,
        _ => theme.accent,
    }
}

/// The fill and text of a state's badge, as the items' (`items::outcome::label`).
pub(super) fn status_badge(status: &OpStatus, theme: OxydeTheme) -> (Hsla, Hsla) {
    match status {
        OpStatus::Error(_) => (theme.danger_background, theme.text_on_danger),
        OpStatus::Completed => (theme.success_text.opacity(0.16), theme.success_text),
        OpStatus::Paused | OpStatus::Cancelled => (theme.badge_surface, theme.text_muted),
        _ => (theme.accent, theme.text_on_accent),
    }
}

pub(super) fn status_label(status: &OpStatus) -> Message {
    match status {
        OpStatus::Queued => Message::OpsQueued,
        OpStatus::Calculating => Message::OpsCalculating,
        OpStatus::Running => Message::OpsRunning,
        OpStatus::Paused => Message::OpsPaused,
        OpStatus::Cancelled => Message::OpsCancelled,
        OpStatus::Completed => Message::OpsCompleted,
        OpStatus::Error(_) => Message::OpsError,
    }
}

pub(super) fn kind(op_type: &FileOpType) -> (Message, &'static str) {
    match op_type {
        FileOpType::Copy => (Message::OpCopy, "icons/copy.svg"),
        FileOpType::Move => (Message::OpMove, "icons/arrow-right.svg"),
        FileOpType::Delete => (Message::OpDelete, "icons/delete.svg"),
        FileOpType::Trash => (Message::OpDelete, "icons/trash.svg"),
        FileOpType::Compress => (Message::OpCompress, "icons/archive.svg"),
        FileOpType::Extract => (Message::OpExtract, "icons/archive-restore.svg"),
    }
}

/// "name" for one source, "name and N others" for several.
pub(super) fn source_summary(first: Option<&Path>, count: usize, cx: &App) -> Option<SharedString> {
    let first = first?;
    let name = first
        .file_name()
        .unwrap_or(first.as_os_str())
        .to_string_lossy()
        .into_owned();
    let others = count.saturating_sub(1);
    if others == 0 {
        return Some(name.into());
    }
    let mut args = FluentArgs::new();
    args.set("name", name);
    args.set("count", others as i64);
    args.set("number", format_number(others as u64, cx.i18n().language()));
    Some(cx.i18n().text_with_args(Message::OpsSourceSummary, &args))
}

/// Where the sources come from or go, opened in the main window by a click.
/// `item` is selected there, as when the archive a compression wrote is shown.
fn place(
    id: &'static str,
    message: Message,
    folder: PathBuf,
    item: Option<PathBuf>,
    cx: &mut Context<OperationsPanelView>,
) -> Stateful<Div> {
    let theme = *cx.theme();
    let shown = item.as_deref().unwrap_or(&folder).to_path_buf();
    let mut args = FluentArgs::new();
    args.set("path", crate::ui::format::display_paths(&shown.to_string_lossy()));
    div()
        .id(id)
        .flex()
        .items_start()
        .gap(rems(0.5))
        .text_color(theme.text_muted)
        .hover(move |style| style.text_color(theme.accent))
        .on_click(cx.listener(move |this, _, _, cx| {
            this.show_in_main_window(folder.clone(), item.clone(), cx);
        }))
        .child(
            svg()
                .path("icons/folder.svg")
                .size(rems(0.9375))
                .mt(rems(0.0625))
                .flex_none()
                .text_color(theme.text),
        )
        .child(div().min_w_0().child(cx.i18n().text_with_args(message, &args)))
}

impl OperationsPanelView {
    /// The selected operation's kind, sources and state.
    pub(super) fn render_heading(&self, operation: &FileOperation, cx: &mut Context<Self>) -> Div {
        let theme = *cx.theme();
        let (kind, icon) = kind(&operation.op_type);
        let (fill, text) = status_badge(&operation.status, theme);
        let summary = source_summary(
            operation.first_sources.first().map(PathBuf::as_path),
            operation.sources.len(),
            cx,
        );
        div()
            .flex_none()
            .px(rems(1.0))
            .pt(rems(1.0))
            .flex()
            .items_center()
            .gap(rems(0.5))
            .child(svg().path(icon).size(rems(1.0)).flex_none().text_color(theme.text))
            .child(div().flex_none().font_weight(FontWeight::SEMIBOLD).child(cx.i18n().text(kind)))
            .when_some(summary, |heading, summary| {
                heading.child(div().min_w_0().truncate().text_color(theme.text_muted).child(summary))
            })
            .child(div().flex_1())
            .child(super::items::outcome::badge(cx.i18n().text(status_label(&operation.status)), fill, text))
    }

    /// Progress, places and failure of the selected operation.
    pub(super) fn render_overview(&self, operation: &FileOperation, cx: &mut Context<Self>) -> Div {
        let theme = *cx.theme();
        let compress = matches!(operation.op_type, FileOpType::Compress);
        let mut places = div().flex().flex_col().gap(rems(0.375));
        if let Some(parent) = &operation.source_parent {
            places = places.child(place("operation-from", Message::OpsFrom, parent.clone(), None, cx));
        }
        if let Some(destination) = &operation.destination {
            // A compression goes to an archive: its folder opens with the archive selected.
            let (folder, item) = match (compress, destination.parent()) {
                (true, Some(parent)) => (parent.to_path_buf(), Some(destination.clone())),
                _ => (destination.clone(), None),
            };
            places = places.child(place("operation-to", Message::OpsTo, folder, item, cx));
        }
        div()
            .flex_none()
            .p(rems(1.0))
            .flex()
            .flex_col()
            .gap(rems(0.875))
            .child(metrics::render(operation, cx))
            .child(places)
            .when_some(
                match &operation.status {
                    OpStatus::Error(error) => Some(error.clone()),
                    _ => None,
                },
                |overview, error| {
                    overview.child(
                        div()
                            .p(rems(0.625))
                            .rounded(rems(0.375))
                            .bg(theme.error.opacity(0.08))
                            .text_color(theme.error)
                            .child(crate::ui::format::error_detail(&error, cx.i18n())),
                    )
                },
            )
    }

    /// Opens `folder` in the main window, in a new tab unless the active one shows
    /// it, selects `item` there and brings the window forward.
    pub(super) fn show_in_main_window(&self, folder: PathBuf, item: Option<PathBuf>, cx: &mut Context<Self>) {
        let Some(app) = self.app.upgrade() else { return };
        let window = app.update(cx, |app, cx| {
            app.show_in_main_window(
                folder.to_string_lossy().into_owned(),
                item.map(|item| item.to_string_lossy().into_owned()),
                cx,
            )
        });
        if let Some(window) = window {
            cx.defer(move |cx| {
                let _ = window.update(cx, |_, window, _| window.activate_window());
            });
        }
    }
}
