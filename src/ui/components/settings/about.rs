use crate::ui::app::OxydeApp;
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::{OxydeTheme, ThemeContext};
use gpui::prelude::*;
use gpui::*;
use crate::ui::i18n::FluentArgs;
use super::controls::{icon, section, separator};

pub(super) fn about_column(view: &OxydeApp, handle: Entity<OxydeApp>, about_width: f32, cx: &App) -> impl IntoElement {
    let theme = *cx.theme();
    let i18n = cx.i18n();
    let mut version = FluentArgs::new();
    version.set("version", env!("CARGO_PKG_VERSION"));
    div()
        .id("settings-about-column")
        .w(rems(about_width)).flex_none()
        .flex().flex_col().gap(rems(0.5))
        .pl(rems(0.75)).pr(rems(0.25))
        .border_l(rems(0.0625)).border_color(theme.border)
        .child(section(i18n.text(Message::SettingsAbout), theme))
        .child(div().flex().flex_col().items_center().gap(rems(0.5)).py(rems(0.75))
            .child(img("icons/logo.png").size(rems(4.5)).flex_none())
            .child(div().text_size(rems(1.5)).font_weight(FontWeight::BOLD).child("Oxyde"))
            .child(div().text_color(theme.text_muted)
                .child(i18n.text_with_args(Message::SettingsVersion, &version))))
        .child(update_row(view, handle.clone(), theme, cx))
        .child(about_link("about-repository", i18n.text(Message::AboutRepository),
            "icons/github.svg", "https://github.com/Carlton2001/Oxyde_FM",
            handle.clone(), theme))
        .child(separator(theme))
        .child(section(i18n.text(Message::AboutMainTools), theme))
        .child(about_tool("about-rust", "Rust", "icons/rust.svg",
            "https://www.rust-lang.org", false, handle.clone(), theme))
        .child(about_tool("about-gpui", "GPUI", "icons/zed.svg",
            "https://www.gpui.rs", false, handle.clone(), theme))
        .child(about_tool("about-lucide", "Lucide",
            if view.theme_id.is_light() { "icons/lucide-light.svg" } else { "icons/lucide-dark.svg" },
            "https://lucide.dev", true, handle.clone(), theme))
}

/// Reflects the update check; each state offers its natural next action.
fn update_row(view: &OxydeApp, handle: Entity<OxydeApp>, theme: OxydeTheme, cx: &App) -> impl IntoElement {
    use crate::ui::app::UpdateStatus;
    let i18n = cx.i18n();
    let (label, icon_path, install) = match &view.update_status {
        UpdateStatus::Idle => (i18n.text(Message::UpdateCheck), "icons/refresh-cw.svg", Some(false)),
        UpdateStatus::Checking => (i18n.text(Message::UpdateChecking), "icons/refresh-cw.svg", None),
        UpdateStatus::Current => (i18n.text(Message::UpdateCurrent), "icons/circle-check.svg", Some(false)),
        UpdateStatus::Failed => (i18n.text(Message::UpdateCheckFailed), "icons/refresh-cw.svg", Some(false)),
        UpdateStatus::Downloading => (i18n.text(Message::UpdateDownloading), "icons/download.svg", None),
        UpdateStatus::Available(update) => {
            let mut args = FluentArgs::new();
            args.set("version", update.version.clone());
            let message = if oxyde_lib::app_paths::is_portable() { Message::UpdateDownloadPortable } else { Message::UpdateInstall };
            (i18n.text_with_args(message, &args), "icons/download.svg", Some(true))
        }
    };
    let row = div().id("about-update").flex().items_center().gap(rems(0.5))
        .px(rems(0.75)).py(rems(0.5)).rounded_md()
        .bg(theme.control_hover)
        .child(icon(icon_path, theme))
        .child(div().flex_1().flex_shrink_0().whitespace_nowrap().child(label));
    // Busy states have no callback, not only a different look.
    let Some(install) = install else { return row.text_color(theme.text_muted).into_any_element(); };
    let action = std::rc::Rc::new(move |_: &mut Window, cx: &mut App| {
        handle.update(cx, |this, cx| if install { this.install_update(cx) } else { this.check_for_updates(true, cx) });
    });
    let pointer = action.clone();
    let row = row.hover(|s| s.bg(theme.menu_hover))
        .on_mouse_down(MouseButton::Left, move |_, window, cx| pointer(window, cx));
    crate::ui::components::popup::keyboard::item(row, action, None, None).into_any_element()
}

/// Links use the platform worker because the shell can pump native messages.
fn about_link(
    id: &'static str, label: SharedString, icon_path: &'static str,
    url: &'static str, handle: Entity<OxydeApp>, theme: OxydeTheme,
) -> impl IntoElement {
    let action = std::rc::Rc::new(move |_: &mut Window, cx: &mut App| { handle.update(cx, |this, cx| this.open_external_item(url.into(), cx)); });
    let pointer = action.clone();
    let row = div().id(id).flex().items_center().gap(rems(0.5))
        .px(rems(0.75)).py(rems(0.5)).rounded_md()
        .bg(theme.control_hover).hover(|s| s.bg(theme.menu_hover))
        .child(icon(icon_path, theme))
        .child(div().flex_1().flex_shrink_0().whitespace_nowrap().child(label))
        .child(icon("icons/external-link.svg", theme))
        .on_mouse_down(MouseButton::Left, move |_, window, cx| pointer(window, cx));
    crate::ui::components::popup::keyboard::item(row, action, None, None)
}

fn about_tool(
    id: &'static str, name: &'static str, logo: &'static str, url: &'static str,
    color_logo: bool, handle: Entity<OxydeApp>, theme: OxydeTheme,
) -> impl IntoElement {
    let action = std::rc::Rc::new(move |_: &mut Window, cx: &mut App| { handle.update(cx, |this, cx| this.open_external_item(url.into(), cx)); });
    let pointer = action.clone();
    let row = div().id(id).flex().items_center().justify_center().gap(rems(0.75))
        .py(rems(0.625)).px(rems(0.75)).rounded_md()
        .border(rems(0.0625)).border_color(theme.border)
        .hover(|s| s.bg(theme.menu_hover))
        .child(if color_logo {
            img(logo).size(rems(1.75)).into_any_element()
        } else {
            svg().path(logo).size(rems(1.75)).text_color(theme.text).into_any_element()
        })
        .child(div().text_size(rems(1.125)).child(name))
        .on_mouse_down(MouseButton::Left, move |_, window, cx| pointer(window, cx));
    crate::ui::components::popup::keyboard::item(row, action, None, None)
}
