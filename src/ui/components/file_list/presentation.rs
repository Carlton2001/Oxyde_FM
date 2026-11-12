use crate::ui::elements::bar_button;
use crate::ui::components::tooltip::NativeTooltipExt;
use crate::ui::app::OxydeApp;
use crate::ui::components::tooltip::Tooltip;
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::theme::OxydeTheme;
use crate::ui::workspace::ViewMode;
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::models::FileEntry;

fn grouping_button(pane_id: &str, enabled: bool, handle: Entity<OxydeApp>, theme: OxydeTheme) -> impl IntoElement {
    let pane_id = pane_id.to_string();
    let tooltip_handle = handle.clone();
    let tooltip_pane = pane_id.clone();
    let reset_handle = handle.clone();
    let reset_pane = pane_id.clone();
    let anchor_handle = handle.clone();
    let anchor_key = format!("grouping-{pane_id}");
    let cell = bar_button::cell(SharedString::from(format!("grouping-{pane_id}")), crate::ui::style::STATUS_BAR_HEIGHT);
    bar_button::fill(bar_button::labelled(cell, rems(0.125)), true, enabled, theme)
        .relative()
        .fixed_tooltip(move |window, cx| {
            use crate::ui::date_groups::GroupMode;
            let mode = tooltip_handle.read(cx).state.find_pane(&tooltip_pane)
                .map(|pane| pane.active_tab().grouping()).unwrap_or(GroupMode::None);
            let label = mode.message();
            let mut args = FluentArgs::new();
            args.set("mode", cx.i18n().text(label).to_string());
            let title = if mode == GroupMode::None {
                cx.i18n().text(Message::Grouping)
            } else {
                cx.i18n().text_with_args(Message::GroupingCurrent, &args)
            };
            Tooltip::with_secondary_action(
                title,
                cx.i18n().text(Message::GroupNone),
                cx.i18n().text(Message::MouseRightClick),
            )(window, cx)
        })
        .on_mouse_down(MouseButton::Left, move |event, _, cx| {
            cx.stop_propagation();
            handle.update(cx, |view, cx| {
                let key = format!("grouping-{pane_id}");
                if view.close_menu_owned_by(&key, cx) { return; }
                let anchor = view.context_anchor_points.get(&key).copied().unwrap_or(event.position);
                view.open_grouping_menu(&pane_id, anchor, cx);
                view.mark_menu_owner(key);
            });
        })
        .on_mouse_down(MouseButton::Right, move |_, _, cx| {
            cx.stop_propagation();
            reset_handle.update(cx, |view, cx| view.set_grouping(&reset_pane, crate::ui::date_groups::GroupMode::None, cx));
        })
        .on_mouse_up(MouseButton::Right, |_, _, cx| cx.stop_propagation())
        .child(canvas(|bounds, _, _| bounds, move |_, bounds, _, cx| {
            anchor_handle.update(cx, |view, _| {
                view.context_anchor_points.insert(anchor_key.clone(), point(bounds.left(), bounds.top()));
            });
        }).absolute().left_0().top_0().size_full())
        .child(bar_button::icon("icons/layers.svg", bar_button::icon_color(true, enabled, theme)))
        .child(crate::ui::elements::button::menu_chevron(true, if enabled { theme.accent } else { theme.text_muted }))
}

fn sizes_button(pane_id: &str, active: bool, handle: Entity<OxydeApp>, theme: OxydeTheme, cx: &mut Context<OxydeApp>) -> impl IntoElement {
    let pane_id = pane_id.to_string();
    bar_button::fill(bar_button::cell(SharedString::from(format!("sizes-{pane_id}")), crate::ui::style::STATUS_BAR_HEIGHT), true, active, theme)
        .fixed_tooltip(Tooltip::with_shortcut(cx.i18n().text(Message::ToolSizes), Some("Alt+Shift+S".into())))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            cx.stop_propagation();
            handle.update(cx, |view, cx| {
                view.state.active_pane_id = pane_id.clone();
                view.calculate_sizes(cx);
            });
        })
        .child(bar_button::icon("icons/chart-bar.svg", bar_button::icon_color(true, active, theme)))
}

/// Returns the display type label for a file entry.
/// Symlinks and junctions are identified before extension/dir detection.
pub(crate) fn file_type_label(f: &FileEntry) -> String {
    if network_entry(f) { return "Réseau".to_string(); }
    if f.is_broken_link {
        return "BrokenLink".to_string();
    }
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
    } else if f.is_broken_link {
        Some(Message::FileTypeBrokenLink)
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

pub(crate) fn network_category(file: &FileEntry, i18n: &crate::ui::i18n::I18n) -> String {
    // GVfs announces services, not machines: the protocol is what tells them apart.
    if !cfg!(target_os = "windows") {
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("protocol", network_protocol(&file.path));
        return i18n.text_with_args(Message::NetworkService, &args).to_string();
    }
    if let Some(category) = file.network_details.as_ref().map(|d| &d.category).filter(|category| !category.is_empty()) {
        return category.clone();
    }
    i18n.text(if oxyde_lib::commands::io::is_network_server(&file.path) { Message::NetworkComputer } else { Message::NetworkOtherDevices }).to_string()
}

/// The protocol of a GVfs location, named as users meet it.
pub(crate) fn network_protocol(uri: &str) -> String {
    let scheme = uri.split_once("://").map_or("", |(scheme, _)| scheme).to_ascii_lowercase();
    match scheme.trim_end_matches("+sd") {
        "dav" | "davs" => "WebDAV".into(),
        "ftps" => "FTP".into(),
        other => other.to_ascii_uppercase(),
    }
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
            crate::ui::elements::checkbox::checkbox(theme, selected, false),
        )
}

/// The buttons closing the status bar: the size bars and grouping toggles as
/// full-height cells, then, past a separator, the switch between details and grid.
pub(super) fn status_actions(
    pane_id: &str,
    sizes_active: bool,
    grouped: bool,
    active_mode: ViewMode,
    handle: Entity<OxydeApp>,
    theme: OxydeTheme,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    div()
        .h_full()
        .flex()
        .flex_none()
        .items_center()
        .pr(rems(0.5))
        .child(sizes_button(pane_id, sizes_active, handle.clone(), theme, cx))
        .child(grouping_button(pane_id, grouped, handle.clone(), theme))
        .child(div().flex_none().w(rems(0.0625)).h(rems(1.0)).mx_2().bg(theme.border))
        .child(view_mode_switch(pane_id, active_mode, handle, theme, cx))
}

/// Details and grid as one segmented switch in the status bar.
fn view_mode_switch(
    pane_id: &str,
    active_mode: ViewMode,
    handle: Entity<OxydeApp>,
    theme: OxydeTheme,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let mut switch = crate::ui::elements::segmented::segmented(
        SharedString::from(format!("view-mode-{pane_id}")),
        None,
        theme,
    );
    for (mode, name, icon, tooltip) in [
        (ViewMode::Details, "details", "icons/list.svg", Message::ViewDetails),
        (ViewMode::Grid, "grid", "icons/layout-grid.svg", Message::ViewGrid),
    ] {
        let selected = mode == active_mode;
        let pane_id = pane_id.to_string();
        let handle = handle.clone();
        switch = switch.child(
            crate::ui::elements::segmented::segment(
                SharedString::from(format!("view-mode-{pane_id}-{name}")),
                selected,
                true,
                theme,
            )
            .w(rems(1.75))
            .px_0()
            .fixed_tooltip(Tooltip::text(cx.i18n().text(tooltip)))
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
            .child(svg().path(icon).size(rems(0.875)).text_color(if selected { theme.accent } else { theme.text })),
        );
    }
    switch
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

    #[::core::prelude::v1::test]
    fn gvfs_services_are_named_by_protocol() {
        for (uri, protocol) in [
            ("smb://DESKTOP.local/", "SMB"),
            ("dav+sd://files%20on%20host._webdav._tcp.local", "WebDAV"),
            ("davs://host/", "WebDAV"),
            ("sftp://host/", "SFTP"),
            ("ftps://host/", "FTP"),
            ("afp://mac/", "AFP"),
        ] {
            assert_eq!(network_protocol(uri), protocol);
        }
        #[cfg(not(target_os = "windows"))]
        {
            let i18n = crate::ui::i18n::I18n::for_language(crate::ui::i18n::Language::French);
            let file = FileEntry { name: "nas".into(), path: "smb://nas/".into(), is_dir: true, ..Default::default() };
            assert_eq!(network_category(&file, &i18n).replace(['\u{2068}', '\u{2069}'], ""), "Partage SMB");
        }
    }
}
