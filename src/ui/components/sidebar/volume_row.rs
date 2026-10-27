//! Unmounted volume row: dimmed after the mounted drives; a left click mounts it.
use super::sidebar_item_height;
use crate::ui::app::OxydeApp;
use crate::ui::components::tooltip::hover::{HoverRegion, SharedTooltipExt};
use crate::ui::components::tooltip::Tooltip;
use crate::ui::i18n::I18nContext;
use crate::ui::theme::OxydeTheme;
use crate::ui::workspace::SidebarItem;
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::platform::UnmountedVolume;

pub(super) fn name(volume: &UnmountedVolume, i18n: &crate::ui::i18n::I18n) -> String {
    if volume.label.is_empty() {
        crate::ui::format::unlabeled_volume(volume.size, i18n)
    } else {
        volume.label.clone()
    }
}

pub(super) fn render(
    item: &SidebarItem,
    volume: &UnmountedVolume,
    row_w: f32,
    show_tooltips: bool,
    mounting: bool,
    h: Entity<OxydeApp>,
    theme: OxydeTheme,
    cx: &mut App,
) -> AnyElement {
    let label = name(volume, cx.i18n());
    let size = crate::ui::format::format_size(volume.size, false, false, cx.i18n().language());
    let details = [volume.file_system.as_deref(), Some(size.as_str())].into_iter().flatten().collect::<Vec<_>>().join(" · ");
    let id = volume.id.clone();
    div()
        .id(SharedString::from(format!("sb-vol-{}", volume.id)))
        .w(px(row_w))
        .h(px(sidebar_item_height(item)))
        .px_3()
        .flex()
        .items_center()
        .hover(move |style| style.bg(theme.hover))
        .when(show_tooltips, |row| row.shared_tooltip(
            h.clone(), HoverRegion::DirectoryTree, format!("volume:{}", volume.id), cx,
            Tooltip::text(format!("{label}\n{details}")),
        ))
        .on_mouse_down(MouseButton::Left, {
            let h = h.clone();
            move |_, _, cx| h.update(cx, |this, cx| this.mount_volume(id.clone(), cx))
        })
        // Keeps the label aligned with the drives, which have a chevron.
        .child(div().w_4().h_4().mr_1().flex_shrink_0())
        .child(
            div().w_4().h_4().mr_2().flex().items_center().justify_center().flex_shrink_0()
                .child(svg()
                    .path(if volume.removable { "icons/usb.svg" } else { "icons/hard-drive.svg" })
                    .size_4()
                    .text_color(theme.text_muted)),
        )
        .child(
            div().flex_1().min_w_0().truncate()
                .text_color(theme.text_muted)
                .when(mounting, |label| label.opacity(0.6))
                .child(label),
        )
        .into_any_element()
}
