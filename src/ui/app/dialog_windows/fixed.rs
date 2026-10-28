//! Dialogs locked over the main window: rename, compression, network drives, remote
//! credentials, confirmations and operation notices.
use super::DialogKind;
use crate::ui::app::OxydeApp;
use crate::ui::components::modal::{action_button, Modal};
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
            Self::Rename | Self::Network | Self::RemoteLogin => 30.0,
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
            Self::TrashDelete => app.pending_trash_delete = None,
            Self::SettingsConfirmation => app.resolve_settings_confirmation(false, window, cx),
            Self::Notice => app.file_operation_notice = None,
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
            Self::TrashDelete => app.pending_trash_delete = None,
            Self::SettingsConfirmation => app.settings_confirmation = None,
            Self::Notice => app.file_operation_notice = None,
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
        use crate::ui::components::{network_dialog, remote_login_dialog, rename_dialog, settings};
        match self {
            Self::Rename => rename_dialog::render_rename_dialog(app, owner, cx).into_any_element(),
            Self::Network => network_dialog::render_network_dialog(app, owner, window, cx).into_any_element(),
            Self::NetworkForce => network_dialog::render_force_confirmation(app, owner, window, cx).into_any_element(),
            Self::RemoteLogin => remote_login_dialog::render_remote_login_dialog(app, owner, window, cx).into_any_element(),
            Self::TrashDelete => super::super::trash_settings::render_delete_confirmation(app, owner, window, cx).into_any_element(),
            Self::SettingsConfirmation => settings::render_settings_confirmation(app, owner, window, cx).into_any_element(),
            Self::Notice => render_notice(app, owner, focus, cx).into_any_element(),
            Self::Trash | Self::Conflict => div().into_any_element(),
        }
    }
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
    let mut args = FluentArgs::new();
    args.set("detail", crate::ui::format::error_detail(&detail, cx.i18n()));
    let message = cx.i18n().text_with_args(key, &args);
    let theme = *cx.theme();
    let keyboard = owner.clone();
    let close = owner.clone();
    let button = owner;
    let dismiss = |handle: &Entity<OxydeApp>, cx: &mut App| {
        handle.update(cx, |app, cx| {
            app.file_operation_notice = None;
            cx.notify();
        });
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
        .child(
            Modal::new(
                "operation-notice",
                cx.i18n().text(Message::OperationFailed),
                div().text_color(theme.text_muted).child(message),
            )
            .title_icon("icons/circle-x.svg")
            .standalone(false)
            .on_close(move |_, cx| dismiss(&close, cx))
            .action(
                action_button("operation-notice-close")
                    .hover(|d| d.bg(theme.control_hover))
                    .child(cx.i18n().text(Message::Close))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| dismiss(&button, cx)),
            ),
        )
}
