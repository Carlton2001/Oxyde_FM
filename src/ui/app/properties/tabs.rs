use super::{PropertiesDialog, PropertiesTab, PropertiesWindow};
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::OxydeTheme;
use gpui::prelude::FluentBuilder;
use crate::ui::elements::segmented::{segment, segmented};
use gpui::*;

/// General first, then the tab the item has: Shortcut on Windows, Permissions
/// elsewhere, as one segmented control centered under the title.
pub(super) fn render(
    dialog: &PropertiesDialog,
    handle: &Entity<PropertiesWindow>,
    has_shortcut: bool,
    has_permissions: bool,
    theme: OxydeTheme,
    cx: &mut Context<PropertiesWindow>,
) -> Option<Div> {
    if !has_shortcut && !has_permissions {
        return None;
    }
    let mut control = segmented("properties-tabs", None, theme);
    for (tab, id, message, icon, shown) in [
        (PropertiesTab::General, "properties-general-tab", Message::PropertiesGeneral, "icons/file-text.svg", true),
        (PropertiesTab::Shortcut, "properties-shortcut-tab", Message::PropertiesShortcut, "icons/link.svg", has_shortcut),
        (PropertiesTab::Permissions, "properties-permissions-tab", Message::PropertiesPermissions, "icons/key.svg", has_permissions),
    ] {
        if !shown {
            continue;
        }
        let select = handle.clone();
        let active = tab == dialog.tab;
        control = control.child(
            segment(id, active, true, theme)
                .px(rems(0.875))
                .child(svg().path(icon).size(rems(0.875)).flex_none().text_color(if active { theme.accent } else { theme.text }))
                .child(cx.i18n().text(message))
                .when(!active, |segment| {
                    segment.on_click(move |_, window, cx| {
                        cx.stop_propagation();
                        select.update(cx, |this, cx| this.select_properties_tab(tab, window, cx));
                    })
                }),
        );
    }
    Some(div().flex().flex_none().justify_center().px(rems(1.0)).pt(rems(0.75)).child(control))
}
