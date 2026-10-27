use super::{PropertiesDialog, PropertiesTab, PropertiesWindow};
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::OxydeTheme;
use gpui::prelude::FluentBuilder;
use gpui::*;

/// General first, then the tabs the item has: Shortcut on Windows, Permissions
/// elsewhere, and Contents for folders whose permissions may change.
pub(super) fn render(
    dialog: &PropertiesDialog,
    handle: &Entity<PropertiesWindow>,
    has_shortcut: bool,
    has_permissions: bool,
    has_contents: bool,
    theme: OxydeTheme,
    cx: &mut Context<PropertiesWindow>,
) -> Option<Div> {
    if !has_shortcut && !has_permissions {
        return None;
    }
    let mut tabs = div()
        .flex()
        .flex_none()
        .items_center()
        .px(rems(1.25))
        .bg(theme.dialog_tabs)
        .gap(rems(0.25))
        .border_b(rems(0.0625))
        .border_color(theme.border);
    for (tab, id, message, icon, shown) in [
        (PropertiesTab::General, "properties-general-tab", Message::PropertiesGeneral, "icons/file-text.svg", true),
        (PropertiesTab::Shortcut, "properties-shortcut-tab", Message::PropertiesShortcut, "icons/link.svg", has_shortcut),
        (PropertiesTab::Permissions, "properties-permissions-tab", Message::PropertiesPermissions, "icons/key.svg", has_permissions),
        (PropertiesTab::Contents, "properties-contents-tab", Message::PropertiesContents, "icons/folder-tree.svg", has_contents),
    ] {
        if !shown {
            continue;
        }
        let select = handle.clone();
        let active = tab == dialog.tab;
        tabs = tabs.child(
            div()
                .id(id)
                .px(rems(1.0))
                .py(rems(0.75))
                .text_size(rems(0.8125))
                .font_weight(FontWeight::MEDIUM)
                .flex()
                .items_center()
                .justify_center()
                .gap(rems(0.5))
                .when(!active, |tab| {
                    tab.hover(move |style| style.bg(theme.hover).text_color(theme.text))
                })
                .child(svg().path(icon).size(rems(0.875)).flex_none().text_color(
                    if active { theme.accent } else { theme.text },
                ))
                .rounded_none()
                .border_b(rems(0.0625))
                .border_color(if active { theme.accent } else { transparent_black() })
                .text_color(if active { theme.accent } else { theme.text_muted })
                .child(cx.i18n().text(message))
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    select.update(cx, |this, cx| this.select_properties_tab(tab, window, cx));
                }),
        );
    }
    Some(tabs)
}
