use crate::ui::elements::button::{button_style, ButtonVariant};
use crate::ui::components::tooltip::NativeTooltipExt;
use crate::ui::app::OxydeApp;
use crate::ui::components::tooltip::Tooltip;
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::theme::OxydeTheme;
use crate::ui::workspace::ViewMode;
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::models::FileEntry;

pub(super) fn grouping_button(pane_id: &str, enabled: bool, handle: Entity<OxydeApp>, theme: OxydeTheme) -> impl IntoElement {
    let pane_id = pane_id.to_string();
    let tooltip_handle = handle.clone();
    let tooltip_pane = pane_id.clone();
    let reset_handle = handle.clone();
    let reset_pane = pane_id.clone();
    let anchor_handle = handle.clone();
    let anchor_key = format!("grouping-{pane_id}");
    button_style(
        div().id(SharedString::from(format!("grouping-{pane_id}"))),
        theme, ButtonVariant::Ghost, true,
    )
        .relative().h(rems(1.5)).px(rems(0.25)).gap(rems(0.125)).flex_none().flex().items_center().justify_center().rounded_md()
        .bg(if enabled { theme.selected } else { theme.header })
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
        .child(svg().path("icons/layers.svg").size(rems(1.0)).text_color(if enabled { theme.accent } else { theme.text }))
        .child(crate::ui::elements::button::menu_chevron(true, if enabled { theme.accent } else { theme.text_muted }))
}

pub(super) fn sizes_button(pane_id: &str, active: bool, handle: Entity<OxydeApp>, theme: OxydeTheme, cx: &mut Context<OxydeApp>) -> impl IntoElement {
    let pane_id = pane_id.to_string();
    button_style(
        div().id(SharedString::from(format!("sizes-{pane_id}"))),
        theme, ButtonVariant::Ghost, true,
    )
        .size(rems(1.5)).flex_none().flex().items_center().justify_center().rounded_md()
        .bg(if active { theme.selected } else { theme.header })
        .fixed_tooltip(Tooltip::text(cx.i18n().text(Message::ToolSizes)))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            cx.stop_propagation();
            handle.update(cx, |view, cx| {
                view.state.active_pane_id = pane_id.clone();
                view.calculate_sizes(cx);
            });
        })
        .child(svg().path("icons/chart-bar.svg").size(rems(1.0)).text_color(if active { theme.accent } else { theme.text }))
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
            crate::ui::elements::checkbox::checkbox(theme, selected, false),
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
