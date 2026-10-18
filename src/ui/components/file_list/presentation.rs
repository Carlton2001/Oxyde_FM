use crate::ui::elements::button::{button_style, ButtonVariant};
use crate::ui::components::tooltip::NativeTooltipExt;
use crate::ui::app::OxydeApp;
use crate::ui::components::tooltip::Tooltip;
use crate::ui::date_groups::DateGroup;
use crate::ui::format::format_number;
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::theme::OxydeTheme;
use crate::ui::workspace::ViewMode;
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::models::FileEntry;

pub(super) fn date_group_header(group: DateGroup, count: usize, collapsed: bool, pane_id: String, handle: Entity<OxydeApp>, height: f32, cx: &mut App) -> AnyElement {
    let theme = *OxydeTheme::global(cx);
    let mut args = FluentArgs::new();
    args.set("count", count);
    args.set("number", format_number(count as u64, cx.i18n().language()));
    div().id(SharedString::from(format!("date-group-{pane_id}-{group:?}"))).flex_none()
        .h(px(height)).w_full().flex().items_center().gap(rems(0.5))
        .px(rems(0.5)).text_size(rems(0.8125)).text_color(theme.text)
        .border_b(rems(0.0625)).border_color(theme.border)
        .hover(|style| style.bg(theme.control_hover))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            cx.stop_propagation();
            handle.update(cx, |view, cx| {
                view.finish_pointer_drag(cx);
                view.activate_pane(&pane_id, cx);
                if let Some(pane) = view.state.find_pane_mut(&pane_id) {
                    let groups = &mut pane.active_tab_mut().collapsed_date_groups;
                    if !groups.remove(&group) { groups.insert(group); }
                }
                cx.notify();
            });
        })
        .child(svg().path(if collapsed { "icons/chevron-right.svg" } else { "icons/chevron-down.svg" }).size(rems(1.0)).text_color(theme.accent))
        .child(cx.i18n().text(group.message()))
        .child(div().rounded_sm().border(rems(0.0625)).border_color(theme.border).px(rems(0.375)).text_size(rems(0.75)).text_color(theme.text_muted).child(cx.i18n().text_with_args(Message::DateGroupCount, &args)))
        .into_any_element()
}

pub(super) fn date_group_button(pane_id: &str, enabled: bool, handle: Entity<OxydeApp>, theme: OxydeTheme, cx: &mut Context<OxydeApp>) -> impl IntoElement {
    let pane_id = pane_id.to_string();
    button_style(
        div().id(SharedString::from(format!("group-by-date-{pane_id}"))),
        theme, ButtonVariant::Ghost, true,
    )
        .size(rems(1.5)).flex_none().flex().items_center().justify_center().rounded_md()
        .bg(if enabled { theme.selected } else { theme.header })
        .native_tooltip(Tooltip::text(cx.i18n().text(Message::SettingsGroupByDate)))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            cx.stop_propagation();
            handle.update(cx, |view, cx| view.toggle_date_grouping(&pane_id, cx));
        })
        .child(svg().path("icons/calendar-days.svg").size(rems(1.0)).text_color(if enabled { theme.accent } else { theme.text }))
}

/// Returns the display type label for a file entry.
/// Symlinks and junctions are identified before extension/dir detection.
pub(crate) fn file_type_label(f: &FileEntry) -> String {
    if network_entry(f) { return "Réseau".to_string(); }
    if f.is_symlink {
        return "Symlink".to_string();
    }
    if f.is_junction {
        return "Junction".to_string();
    }
    if f.is_dir {
        return "Dossier".to_string();
    }
    std::path::Path::new(&f.name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_uppercase())
        .unwrap_or_else(|| "Fichier".to_string())
}

pub(crate) fn file_type_display(f: &FileEntry, i18n: &crate::ui::i18n::I18n) -> String {
    let message = if network_entry(f) {
        Some(Message::SidebarNetwork)
    } else if f.is_symlink {
        Some(Message::FileTypeSymlink)
    } else if f.is_junction {
        Some(Message::FileTypeJunction)
    } else if f.is_dir {
        Some(Message::FileTypeFolder)
    } else if std::path::Path::new(&f.name).extension().is_none() {
        Some(Message::FileTypeFile)
    } else {
        None
    };
    message
        .map(|message| i18n.text(message).to_string())
        .unwrap_or_else(|| file_type_label(f))
}

/// Shell devices and UNC server roots belong to the network neighborhood.
pub(crate) fn network_entry(file: &FileEntry) -> bool {
    file.path.starts_with("::{") || file.path.starts_with('?')
        || file.path.strip_prefix("\\\\").is_some_and(|tail| {
            let server = tail.trim_end_matches('\\');
            !server.is_empty() && !server.contains('\\')
        })
}

pub(super) fn network_category(file: &FileEntry, i18n: &crate::ui::i18n::I18n) -> String {
    if let Some(category) = file.network_details.as_ref().map(|d| &d.category).filter(|category| !category.is_empty()) {
        return category.clone();
    }
    i18n.text(if oxyde_lib::commands::io::is_network_server(&file.path) { Message::NetworkComputer } else { Message::NetworkOtherDevices }).to_string()
}

#[cfg(test)]
mod network_type_tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn network_devices_and_servers_use_localized_network_type_without_relabeling_shares() {
        for language in [crate::ui::i18n::Language::French, crate::ui::i18n::Language::English] {
            let i18n = crate::ui::i18n::I18n::for_language(language);
            for (path, is_dir) in [("::{device}", false), ("?device", false), ("\\\\server", true)] {
                let file = FileEntry { name: "Bbox".into(), path: path.into(), is_dir, ..Default::default() };
                assert_eq!(file_type_display(&file, &i18n), i18n.text(Message::SidebarNetwork).as_ref());
                assert_eq!(file_type_label(&file), "Réseau");
            }
            for path in ["\\\\server\\share", "\\\\server\\share\\folder", "C:\\folder"] {
                let file = FileEntry { name: "folder".into(), path: path.into(), is_dir: true, ..Default::default() };
                assert_eq!(file_type_display(&file, &i18n), i18n.text(Message::FileTypeFolder).as_ref());
            }
        }
    }
}

/// Uppercases only the extension for presentation in grid cards.
pub(super) fn grid_display_name(name: &str) -> String {
    let Some((stem, extension)) = name.rsplit_once('.') else {
        return name.to_string();
    };
    if stem.is_empty() || extension.is_empty() {
        return name.to_string();
    }
    format!("{stem}.{}", extension.to_uppercase())
}

pub(super) fn selection_checkbox(
    selected: bool,
    path: String,
    pane_id: String,
    handle: Entity<OxydeApp>,
    theme: OxydeTheme,
) -> impl IntoElement {
    div()
        .id(SharedString::from(format!("selection-checkbox-{path}")))
        .size(rems(1.5))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(move |_, window, cx| {
            cx.stop_propagation();
            handle.update(cx, |view, cx| {
                view.state.active_pane_id = pane_id.clone();
                view.focus_handle.focus(window, cx);
                view.toggle_selection(&pane_id, path.clone(), true, false, cx);
            });
        })
        .child(
            div()
                .size(rems(1.0))
                .rounded(rems(0.25))
                .border(rems(0.0625))
                .border_color(if selected { theme.accent } else { theme.border })
                .bg(if selected { theme.accent } else { theme.surface })
                .flex()
                .items_center()
                .justify_center()
                .when(selected, |d| d.child(svg().path("icons/check.svg").size(rems(0.75)).text_color(theme.text_on_accent))),
        )
}

pub(super) fn view_mode_button(
    pane_id: &str,
    mode: ViewMode,
    active_mode: ViewMode,
    icon: &'static str,
    tooltip: Message,
    handle: Entity<OxydeApp>,
    theme: OxydeTheme,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let selected = mode == active_mode;
    let pane_id = pane_id.to_string();
    button_style(div()
        .id(SharedString::from(format!(
            "view-mode-{}-{}",
            pane_id,
            if mode == ViewMode::Grid {
                "grid"
            } else {
                "details"
            }
        ))), theme, ButtonVariant::Ghost, true)
        .w(rems(1.5))
        .h(rems(1.5))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .bg(if selected {
            theme.selected
        } else {
            theme.header
        })
        .native_tooltip(Tooltip::text(cx.i18n().text(tooltip)))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            cx.stop_propagation();
            handle.update(cx, |this, cx| {
                if let Some(pane) = this.state.find_pane_mut(&pane_id) {
                    pane.active_tab_mut().view_mode = mode;
                }
                this.save_workspace(cx);
                cx.notify();
            });
        })
        .child(svg().path(icon).size_4().text_color(if selected {
            theme.accent
        } else {
            theme.text
        }))
}

pub(super) fn size_histogram(size: u64, total: u64, color: Hsla) -> Div {
    div().absolute().left_0().top_0().bottom_0().w(relative((size as f64 / total as f64).clamp(0.0, 1.0) as f32)).bg(color.opacity(0.12))
}

pub(super) fn size_loading(path: &str, color: Hsla) -> AnyElement {
    div().flex().items_center().justify_end().gap(rems(0.1875))
        .h(rems(1.0))
        .children((0..3).map(|index| {
            div().size(rems(0.1875)).rounded_full().bg(color)
                .with_animation(
                    SharedString::from(format!("size-loading-{path}-{index}")),
                    Animation::new(std::time::Duration::from_millis(1000)).repeat(),
                    move |dot, delta| {
                        let wave = (1.0 + ((delta - index as f32 / 3.0) * std::f32::consts::TAU).cos()) * 0.5;
                        dot.opacity(0.25 + wave * 0.75)
                    },
                )
        })).into_any_element()
}
