//! Dialogs locked over the main window: rename, compression, network drives, remote
//! credentials, « Open With », confirmations and operation notices.
use super::DialogKind;
use crate::ui::app::OxydeApp;
use crate::ui::components::modal::Modal;
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;

impl DialogKind {
    pub(super) fn is_fixed(self) -> bool {
        !matches!(self, Self::Trash | Self::Conflict)
    }

    /// Initial content width, in rems; the height follows the content.
    pub(super) fn fixed_width(self) -> f32 {
        match self {
            Self::Rename | Self::Network | Self::RemoteLogin | Self::OpenWith => 30.0,
            _ => 27.5,
        }
    }

    pub(super) fn fixed_active(self, app: &OxydeApp) -> bool {
        match self {
            Self::Rename => app.editing_path.is_some() && !app.inline_rename,
            // The force prompt replaces the drive dialog while it is shown.
            Self::Network => app.network_dialog.is_some() && app.network_force_confirmation.is_none(),
            Self::NetworkForce => app.network_force_confirmation.is_some(),
            Self::RemoteLogin => app.remote_login.is_some(),
            Self::ArchivePassword => app.archive_password.is_some(),
            Self::OpenWith => app.open_with.is_some(),
            Self::TrashDelete => app.pending_trash_delete.is_some(),
            Self::SettingsConfirmation => app.settings_confirmation.is_some(),
            Self::Notice => app.file_operation_notice.is_some(),
            Self::Trash | Self::Conflict => false,
        }
    }

    pub(super) fn initial_focus(self, app: &OxydeApp, cx: &App) -> Option<FocusHandle> {
        match self {
            Self::Rename => app.editing_view.as_ref().map(|input| input.read(cx).focus_handle.clone()),
            Self::Network => app.network_dialog.as_ref().map(|dialog| dialog.focus.clone()),
            Self::NetworkForce => app.network_force_confirmation.as_ref().map(|c| c.focus.clone()),
            Self::RemoteLogin => app.remote_login.as_ref().map(|login| login.initial.clone()),
            Self::ArchivePassword => app.archive_password.as_ref().map(|prompt| prompt.input.read(cx).focus_handle.clone()),
            Self::OpenWith => app.open_with.as_ref().map(|prompt| prompt.input.read(cx).focus_handle.clone()),
            Self::TrashDelete => app.pending_trash_delete.as_ref().map(|p| p.focus.clone()),
            Self::SettingsConfirmation => app.settings_confirmation.as_ref().map(|c| c.focus.clone()),
            _ => None,
        }
    }

    /// Title-bar close and Alt+F4. A busy operation refuses to close.
    pub(super) fn request_fixed_close(
        self,
        app: &mut OxydeApp,
        window: &mut Window,
        cx: &mut Context<OxydeApp>,
    ) -> bool {
        match self {
            Self::Rename => {
                if app.rename_pending {
                    return false;
                }
                app.cancel_rename(cx);
            }
            Self::Network => {
                if app.network_dialog.as_ref().is_some_and(|dialog| dialog.busy) {
                    return false;
                }
                app.cancel_network_dialog(window, cx);
            }
            Self::NetworkForce => app.resolve_network_force(false, window, cx),
            Self::RemoteLogin => app.answer_remote_login(true, window, cx),
            Self::ArchivePassword => app.answer_archive_password(true, window, cx),
            Self::OpenWith => app.answer_open_with(false, cx),
            Self::TrashDelete => app.pending_trash_delete = None,
            Self::SettingsConfirmation => app.resolve_settings_confirmation(false, window, cx),
            Self::Notice => app.dismiss_notice(),
            Self::Trash | Self::Conflict => {}
        }
        cx.notify();
        true
    }

    /// Drop the request when its window cannot be created.
    pub(super) fn discard_fixed(self, app: &mut OxydeApp, cx: &mut Context<OxydeApp>) {
        match self {
            Self::Rename => app.cancel_rename(cx),
            Self::Network => app.network_dialog = None,
            Self::NetworkForce => app.network_force_confirmation = None,
            Self::RemoteLogin => app.dismiss_remote_login(),
            Self::ArchivePassword => app.dismiss_archive_password(),
            Self::OpenWith => app.open_with = None,
            Self::TrashDelete => app.pending_trash_delete = None,
            Self::SettingsConfirmation => app.settings_confirmation = None,
            Self::Notice => app.dismiss_notice(),
            Self::Trash | Self::Conflict => {}
        }
    }

    pub(super) fn render_fixed(
        self,
        app: &mut OxydeApp,
        owner: Entity<OxydeApp>,
        focus: &FocusHandle,
        window: &mut Window,
        cx: &mut Context<OxydeApp>,
    ) -> AnyElement {
        use crate::ui::components::{archive_password_dialog, network_dialog, open_with_dialog, remote_login_dialog, rename_dialog, settings};
        match self {
            Self::Rename => rename_dialog::render_rename_dialog(app, owner, window, cx).into_any_element(),
            Self::Network => network_dialog::render_network_dialog(app, owner, window, cx).into_any_element(),
            Self::NetworkForce => network_dialog::render_force_confirmation(app, owner, window, cx).into_any_element(),
            Self::RemoteLogin => remote_login_dialog::render_remote_login_dialog(app, owner, window, cx).into_any_element(),
            Self::ArchivePassword => archive_password_dialog::render_archive_password_dialog(app, owner, window, cx).into_any_element(),
            Self::OpenWith => open_with_dialog::render_open_with_dialog(app, owner, window, cx).into_any_element(),
            Self::TrashDelete => super::super::trash_settings::render_delete_confirmation(app, owner, window, cx).into_any_element(),
            Self::SettingsConfirmation => settings::render_settings_confirmation(app, owner, window, cx).into_any_element(),
            Self::Notice => render_notice(app, owner, focus, cx).into_any_element(),
            Self::Trash | Self::Conflict => div().into_any_element(),
        }
    }
}

impl OxydeApp {
    fn dismiss_notice(&mut self) {
        self.file_operation_notice = None;
        self.delete_retry = None;
    }
}

/// The notice text; a deletion gets the same layout whether one or many items failed.
fn notice_paragraphs(key: Message, detail: &str, retry: bool, theme: crate::ui::theme::OxydeTheme, cx: &App) -> Vec<AnyElement> {
    use crate::ui::components::confirmation::detail as muted;
    let i18n = cx.i18n();
    let batch = oxyde_lib::models::error::DeleteBatchFailure::from_message(detail);
    if batch.is_none() && !retry {
        let mut args = FluentArgs::new();
        args.set("detail", crate::ui::format::error_detail(detail, i18n));
        return vec![div().child(i18n.text_with_args(key, &args)).into_any_element()];
    }
    let (heading, cause) = match &batch {
        Some(batch) => {
            let mut args = FluentArgs::new();
            args.set("failed", crate::ui::format::format_number(batch.failed as u64, i18n.language()));
            args.set("total", crate::ui::format::format_number(batch.total as u64, i18n.language()));
            (i18n.text_with_args(Message::DeleteBatchFailed, &args), batch.first.as_str())
        }
        None => (i18n.text(Message::DeleteSingleFailed), detail),
    };
    let mut paragraphs = vec![div().font_weight(FontWeight::MEDIUM).child(heading).into_any_element()];
    if batch.is_some() {
        paragraphs.push(muted(i18n.text(Message::DeleteBatchProcessed), theme));
    }
    paragraphs.push(
        div()
            .mt(rems(0.5))
            .flex()
            .flex_col()
            .gap(rems(0.25))
            .px(rems(0.75))
            .py(rems(0.5))
            .rounded_md()
            .bg(theme.error.opacity(0.08))
            .child(div().text_size(rems(0.75)).text_color(theme.text_muted).child(i18n.text(Message::DeleteFailureCause)))
            .child(div().text_color(theme.error).child(crate::ui::format::error_detail(cause, i18n)))
            .into_any_element(),
    );
    if retry {
        paragraphs.push(div().mt(rems(0.5)).child(muted(i18n.text(Message::DeleteRetryHint), theme)).into_any_element());
    }
    paragraphs
}

fn render_notice(
    app: &OxydeApp,
    owner: Entity<OxydeApp>,
    focus: &FocusHandle,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let Some((key, detail)) = app.file_operation_notice.clone() else {
        return div();
    };
    let retry = app.delete_retry.as_ref().is_some_and(|retry| retry.detail == detail);
    let deletion = retry || oxyde_lib::models::error::DeleteBatchFailure::from_message(&detail).is_some();
    let theme = *cx.theme();
    let paragraphs = notice_paragraphs(key, &detail, retry, theme, cx);
    let title = cx.i18n().text(if deletion { Message::DeleteFailedTitle } else { Message::OperationFailed });
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
        crate::ui::components::confirmation::body(
            "icons/circle-x.svg",
            crate::ui::components::confirmation::Tone::Danger,
            SharedString::default(),
            paragraphs,
            None,
            theme,
        ),
    )
    .title_icon("icons/circle-x.svg")
    .standalone(false)
    .on_close(move |_, cx| dismiss(&close, cx))
    .action(
        crate::ui::components::confirmation::cancel_button("operation-notice-close", false, cx.i18n().text(Message::Close), theme)
            .on_mouse_down(MouseButton::Left, move |_, _, cx| dismiss(&button, cx)),
    );
    let modal = if retry {
        modal.action(
            crate::ui::components::confirmation::confirm_button("operation-notice-retry", false, crate::ui::components::confirmation::Tone::Neutral, cx.i18n().text(Message::Retry), theme)
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
