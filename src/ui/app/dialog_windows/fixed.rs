//! Dialogs locked over the main window: rename, compression, network drives, remote
//! credentials, « Open With », confirmations and operation notices.
use super::DialogKind;
use crate::ui::app::OxydeApp;
use gpui::prelude::*;
use gpui::*;

mod notice;
use notice::render_notice;

impl DialogKind {
    pub(super) fn is_fixed(self) -> bool {
        !matches!(self, Self::Trash | Self::Conflict)
    }

    /// Initial content width, in rems; the height follows the content.
    pub(super) fn fixed_width(self) -> f32 {
        match self {
            // Forms; confirmations and notices keep the narrower width.
            Self::Rename | Self::Network | Self::RemoteLogin | Self::ArchivePassword | Self::OpenWith => 30.0,
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
            Self::QuitConfirmation => app.quit_confirmation.is_some(),
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
            Self::QuitConfirmation => app.quit_confirmation.as_ref().map(|c| c.focus.clone()),
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
            // Closing it keeps Oxyde open; once cancelling, it stays until the end.
            Self::QuitConfirmation => {
                if app.quit_confirmation.as_ref().is_some_and(|confirmation| confirmation.cancelling) {
                    return false;
                }
                app.resolve_quit_confirmation(false, window, cx);
            }
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
            Self::QuitConfirmation => app.quit_confirmation = None,
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
            Self::Notice => render_notice(app, owner, focus, window, cx).into_any_element(),
            Self::QuitConfirmation => crate::ui::components::quit_confirmation::render(app, owner, window, cx).into_any_element(),
            Self::Trash | Self::Conflict => div().into_any_element(),
        }
    }
}
