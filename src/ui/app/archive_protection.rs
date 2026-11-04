//! The protection a new archive is created with: no password, or a password typed twice.
//! Only ZIP and 7z can be encrypted (AES-256); other formats keep the choice disabled.
use super::*;
use crate::ui::components::input::{TextInput, TextInputOptions};
use oxyde_lib::utils::archive_password::ArchiveSecret;

pub(crate) struct ArchiveProtection {
    pub protected: bool,
    /// Password and confirmation.
    pub inputs: [Entity<TextInput>; 2],
    /// Kept as typed: an input calling back cannot be read while it updates.
    pub values: [String; 2],
    /// Each field's eye shows its characters.
    pub revealed: [bool; 2],
}

/// Formats whose content Oxyde can encrypt.
pub(crate) fn encryptable(extension: &str) -> bool {
    matches!(extension, "zip" | "7z")
}

impl OxydeApp {
    pub(super) fn begin_archive_protection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let handle = cx.entity();
        let inputs = [0, 1].map(|index| {
            let (change, submit, cancel, menu) = (handle.clone(), handle.clone(), handle.clone(), handle.clone());
            cx.new(|cx| {
                TextInput::new(
                    cx,
                    "",
                    String::new(),
                    move |text, _, cx| change.update(cx, |this, cx| {
                        if let Some(protection) = &mut this.archive_protection {
                            protection.values[index] = text.to_string();
                        }
                        this.rename_error = None;
                        cx.notify();
                    }),
                    move |_, cx| submit.update(cx, |this, cx| this.submit_rename(cx)),
                    move |_, cx| cancel.update(cx, |this, cx| this.cancel_rename(cx)),
                    move |input, selected, point, window, cx| {
                        menu.update(cx, |this, cx| this.open_text_context_menu(input, selected, point, window, cx))
                    },
                    TextInputOptions::default(),
                    window,
                )
                .password()
            })
        });
        self.archive_protection = Some(ArchiveProtection {
            protected: false,
            inputs,
            values: Default::default(),
            revealed: [false; 2],
        });
    }

    pub(crate) fn set_archive_protected(&mut self, protected: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(protection) = &mut self.archive_protection else { return };
        protection.protected = protected;
        if protected {
            let focus = protection.inputs[0].read(cx).focus_handle.clone();
            window.focus(&focus, cx);
        }
        self.rename_error = None;
        cx.notify();
    }

    pub(crate) fn toggle_archive_password_reveal(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(protection) = &mut self.archive_protection else { return };
        protection.revealed[index] = !protection.revealed[index];
        let masked = !protection.revealed[index];
        protection.inputs[index].update(cx, |input, cx| input.set_masked(masked, cx));
        cx.notify();
    }

    /// The password the new archive gets, or the message explaining why it cannot.
    pub(super) fn archive_protection_secret(&self, extension: &str) -> Result<Option<ArchiveSecret>, Message> {
        let Some(protection) = self.archive_protection.as_ref().filter(|p| p.protected && encryptable(extension)) else {
            return Ok(None);
        };
        let [password, confirmation] = &protection.values;
        if password.is_empty() {
            return Err(Message::ArchivePasswordEmpty);
        }
        if password != confirmation {
            return Err(Message::ArchivePasswordMismatch);
        }
        ArchiveSecret::new(password.clone()).map(Some).ok_or(Message::ArchivePasswordTooLong)
    }
}
