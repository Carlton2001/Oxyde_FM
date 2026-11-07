//! System integration: Oxyde opening other applications' folders, and choosing their
//! files in the Open and Save dialogs. Each changes other applications, so they sit in
//! a submenu of their own, their explanations shown rather than left to tooltips. Only
//! the options the system allows are listed.
use crate::ui::app::OxydeApp;
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::{OxydeTheme, ThemeContext};
use gpui::prelude::*;
use gpui::*;
use std::rc::Rc;

/// Whether the system lets Oxyde take any of these roles.
pub(super) fn available(view: &OxydeApp) -> bool {
    view.opens_folders.is_some() || view.chooses_files.is_some()
}

pub(super) fn integration_choices(view: &OxydeApp, handle: Entity<OxydeApp>, cx: &App) -> Vec<AnyElement> {
    let theme = *cx.theme();
    let i18n = cx.i18n();
    let mut rows = Vec::new();
    if let Some(opens_folders) = view.opens_folders {
        rows.push(option_row(
            "settings-default-file-manager",
            i18n.text(Message::SettingsDefaultFileManager),
            i18n.text(Message::SettingsDefaultFileManagerDescription),
            opens_folders,
            handle.clone(),
            theme,
            |view, cx| view.toggle_folder_handler(cx),
        ));
    }
    if let Some(chooses_files) = view.chooses_files {
        rows.push(option_row(
            "settings-file-chooser",
            i18n.text(Message::SettingsFileChooser),
            i18n.text(Message::SettingsFileChooserDescription),
            chooses_files,
            handle,
            theme,
            |view, cx| view.toggle_file_chooser_setting(cx),
        ));
    }
    rows
}

/// A checkbox, its label and, below, what it changes.
fn option_row(
    id: &'static str,
    label: SharedString,
    description: SharedString,
    checked: bool,
    handle: Entity<OxydeApp>,
    theme: OxydeTheme,
    on_toggle: impl Fn(&mut OxydeApp, &mut Context<OxydeApp>) + 'static,
) -> AnyElement {
    let action = Rc::new(move |cx: &mut App| handle.update(cx, |view, cx| on_toggle(view, cx)));
    let key_action = action.clone();
    let row = div()
        .id(id)
        .flex_none()
        .px(rems(0.5))
        .py(rems(0.375))
        .flex()
        .items_start()
        .gap(rems(0.5))
        .rounded(rems(0.3125))
        .hover(|style| style.bg(theme.menu_hover))
        // Level with the label's first line.
        .child(div().pt(rems(0.125)).child(crate::ui::elements::checkbox::checkbox(theme, checked, false)))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(rems(0.125))
                .child(div().child(label))
                .child(div().text_size(rems(0.75)).text_color(theme.text_muted).child(description)),
        )
        .on_mouse_down(MouseButton::Left, move |_, _, cx| action(cx));
    crate::ui::components::popup::keyboard::item(row, Rc::new(move |_, cx| key_action(cx)), None, None).into_any_element()
}
