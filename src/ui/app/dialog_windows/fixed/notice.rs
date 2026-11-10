//! Operation notices: a heading, the cause set apart, then what to do next.
use crate::ui::app::OxydeApp;
use crate::ui::components::confirmation::{self, Tone};
use crate::ui::components::modal::Modal;
use crate::ui::format::{display_paths, error_detail, format_number};
use crate::ui::i18n::{FluentArgs, I18n, I18nContext, Message};
use crate::ui::theme::{OxydeTheme, ThemeContext};
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::models::error::{ArchiveRecoveryFailure, DeleteBatchFailure};

impl OxydeApp {
    pub(super) fn dismiss_notice(&mut self) {
        self.file_operation_notice = None;
        self.delete_retry = None;
    }
}

/// The parts of a notice, filled per failure kind and laid out the same way.
struct Notice {
    title: SharedString,
    heading: SharedString,
    note: Option<SharedString>,
    cause: Option<String>,
    locations: Vec<String>,
    more: Option<SharedString>,
    hint: Option<SharedString>,
}

fn notice(key: Message, detail: &str, retry: bool, i18n: &I18n) -> Notice {
    if let Some(recovery) = ArchiveRecoveryFailure::from_message(detail) {
        let more = (recovery.omitted > 0).then(|| {
            let mut args = FluentArgs::new();
            args.set("count", format_number(recovery.omitted, i18n.language()));
            i18n.text_with_args(Message::ArchiveRecoveryMore, &args)
        });
        let cause = crate::ui::format::recovery_cause_is_explained(&recovery.cause).then(|| error_detail(&recovery.cause, i18n));
        let (heading, hint) = if recovery.completed {
            (Message::ArchiveRecoveryCompletedHeading, Message::ArchiveRecoveryCompletedHint)
        } else {
            (Message::ArchiveRecoveryStoppedHeading, Message::ArchiveRecoveryStoppedHint)
        };
        return Notice {
            title: i18n.text(Message::ArchiveRecoveryNotice),
            heading: i18n.text(heading),
            note: None,
            cause,
            locations: recovery.locations.iter().map(|path| display_paths(path)).collect(),
            more,
            hint: Some(i18n.text(hint)),
        };
    }
    let batch = DeleteBatchFailure::from_message(detail);
    if batch.is_some() || retry {
        let (heading, note, cause) = match &batch {
            Some(batch) => {
                let mut args = FluentArgs::new();
                args.set("failed", format_number(batch.failed as u64, i18n.language()));
                args.set("total", format_number(batch.total as u64, i18n.language()));
                (i18n.text_with_args(Message::DeleteBatchFailed, &args), Some(i18n.text(Message::DeleteBatchProcessed)), batch.first.as_str())
            }
            None => (i18n.text(Message::DeleteSingleFailed), None, detail),
        };
        return Notice {
            title: i18n.text(Message::DeleteFailedTitle),
            heading,
            note,
            cause: Some(error_detail(cause, i18n)),
            locations: Vec::new(),
            more: None,
            hint: retry.then(|| i18n.text(Message::DeleteRetryHint)),
        };
    }
    let (title, heading) = match key {
        Message::TransferFailed => (Message::TransferFailedTitle, Message::TransferFailedHeading),
        Message::PrepareFailed => (Message::PrepareFailedTitle, Message::PrepareFailedHeading),
        Message::OpenWithFailed => (Message::OpenWithFailed, Message::OpenWithFailedHeading),
        _ => (Message::OperationFailed, Message::OperationFailedHeading),
    };
    Notice {
        title: i18n.text(title),
        heading: i18n.text(heading),
        note: None,
        cause: (!detail.trim().is_empty()).then(|| error_detail(detail, i18n)),
        locations: Vec::new(),
        more: None,
        hint: None,
    }
}

fn panel(theme: OxydeTheme, tint: Hsla, label: SharedString) -> Div {
    div()
        .mt(rems(0.5))
        .flex()
        .flex_col()
        .gap(rems(0.25))
        .px(rems(0.75))
        .py(rems(0.5))
        .rounded_md()
        .bg(tint.opacity(0.08))
        .child(div().text_size(rems(0.75)).text_color(theme.text_muted).child(label))
}

fn paragraphs(notice: Notice, i18n: &I18n, theme: OxydeTheme) -> Vec<AnyElement> {
    let mut paragraphs = vec![div().font_weight(FontWeight::MEDIUM).child(notice.heading).into_any_element()];
    if let Some(note) = notice.note {
        paragraphs.push(confirmation::detail(note, theme));
    }
    if !notice.locations.is_empty() {
        let mut list = panel(theme, theme.text_muted, i18n.text(Message::ArchiveRecoveryLocations));
        for location in notice.locations {
            list = list.child(div().child(location));
        }
        if let Some(more) = notice.more {
            list = list.child(div().text_size(rems(0.75)).text_color(theme.text_muted).child(more));
        }
        paragraphs.push(list.into_any_element());
    }
    if let Some(cause) = notice.cause {
        paragraphs.push(
            panel(theme, theme.error, i18n.text(Message::DeleteFailureCause))
                .child(div().text_color(theme.error).child(cause))
                .into_any_element(),
        );
    }
    if let Some(hint) = notice.hint {
        paragraphs.push(div().mt(rems(0.5)).child(confirmation::detail(hint, theme)).into_any_element());
    }
    paragraphs
}

pub(super) fn render_notice(
    app: &OxydeApp,
    owner: Entity<OxydeApp>,
    focus: &FocusHandle,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let Some((key, detail)) = app.file_operation_notice.clone() else {
        return div();
    };
    let retry = app.delete_retry.as_ref().is_some_and(|retry| retry.detail == detail);
    let theme = *cx.theme();
    let notice = notice(key, &detail, retry, cx.i18n());
    let title = notice.title.clone();
    let paragraphs = paragraphs(notice, cx.i18n(), theme);
    let keyboard = owner.clone();
    let close = owner.clone();
    let button = owner.clone();
    let retry_button = owner;
    let dismiss = |handle: &Entity<OxydeApp>, cx: &mut App| {
        handle.update(cx, |app, cx| {
            app.dismiss_notice();
            cx.notify();
        });
    };
    let modal = Modal::new(
        "operation-notice",
        title,
        confirmation::body("icons/circle-x.svg", Tone::Danger, SharedString::default(), paragraphs, None, theme),
    )
    .title_icon("icons/circle-x.svg")
    .standalone(false)
    .on_close(move |_, cx| dismiss(&close, cx))
    .action(
        confirmation::cancel_button("operation-notice-close", false, cx.i18n().text(Message::Close), theme)
            .on_mouse_down(MouseButton::Left, move |_, _, cx| dismiss(&button, cx)),
    );
    let modal = if retry {
        modal.action(
            confirmation::confirm_button("operation-notice-retry", false, Tone::Neutral, cx.i18n().text(Message::Retry), theme)
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    retry_button.update(cx, |app, cx| app.retry_failed_delete(cx));
                }),
        )
    } else {
        modal
    };
    div()
        .absolute()
        .inset_0()
        .track_focus(focus)
        .on_key_down(move |event, _, cx| {
            if matches!(event.keystroke.key.as_str(), "escape" | "enter" | "space") {
                dismiss(&keyboard, cx);
            }
            cx.stop_propagation();
        })
        .child(modal)
}
