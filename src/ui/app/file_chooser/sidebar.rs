//! The places beside the entries, copies of the main sidebar's rows
//! (`sidebar/rows.rs`, `sidebar/tree.rs`): quick access folders, the section line,
//! then the volumes. Choosing needs no menus, drops, reordering or folder tree.
use super::{FileChooserWindow, Place};
use crate::ui::components::scrollbar::{render_scrollbar_with_pointer, ScrollbarAxis};
use crate::ui::components::sidebar::sidebar_paths_equal;
use crate::ui::components::tooltip::{NativeTooltipExt, Tooltip};
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::scale::rem_pixels;
use crate::ui::theme::OxydeTheme;
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::models::DriveInfo;

/// The main sidebar's default width.
pub(super) const WIDTH: Rems = rems(16.25);
const ROW_HEIGHT: Rems = rems(1.75);
const DRIVE_HEIGHT: Rems = rems(2.25);
/// The gap above the first entry.
const LIST_TOP: Rems = rems(0.375);

impl FileChooserWindow {
    fn render_place(&self, index: usize, place: &Place, theme: OxydeTheme, cx: &mut Context<Self>) -> AnyElement {
        let path = place.path.clone();
        let is_home = place.home;
        let is_sel = sidebar_paths_equal(&self.folder, &path);
        let hov_bg = if is_sel { theme.selection_hover } else { theme.hover };
        let is_dimmed = place.label.starts_with('.');
        let name = if place.label.is_empty() { cx.i18n().text(Message::SidebarHome).to_string() } else { place.label.clone() };
        div()
            .id(("file-chooser-place", index))
            .w_full()
            .h(ROW_HEIGHT)
            .px_3()
            .flex()
            .items_center()
            .when(is_dimmed, |d| d.opacity(0.5))
            .relative()
            .when(is_sel, |d| d.bg(theme.selected).child(div().absolute().left_0().top_0().bottom_0().w(rems(0.1875)).bg(theme.accent)))
            .hover(move |s| s.bg(hov_bg))
            .native_tooltip(Tooltip::with_meta(name.clone(), path.clone()))
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| this.show_folder(path.clone(), cx)))
            .child(
                div().w_4().h_4().mr_2().flex().items_center().justify_center().flex_shrink_0()
                    .child(svg().path(if is_home { "icons/house.svg" } else { "icons/pin.svg" }).size_4()
                        .text_color(if is_home { theme.text } else { theme.accent })),
            )
            .child(div().flex_1().min_w_0().truncate().text_color(theme.text).child(name))
            .into_any_element()
    }

    fn render_drive(&self, index: usize, drive: &DriveInfo, theme: OxydeTheme, cx: &mut Context<Self>) -> AnyElement {
        let path = drive.path.clone();
        let is_sel = sidebar_paths_equal(&self.folder, &path);
        let hov_bg = if is_sel { theme.selection_hover } else { theme.hover };
        let label = crate::ui::format::drive_name(drive, cx.i18n());
        div()
            .id(("file-chooser-drive", index))
            .w_full()
            .h(DRIVE_HEIGHT)
            .px_3()
            .flex()
            .items_center()
            .relative()
            .when(is_sel, |d| d.bg(theme.selected).child(div().absolute().left_0().top_0().bottom_0().w(rems(0.1875)).bg(theme.accent)))
            .hover(move |s| s.bg(hov_bg))
            .native_tooltip(Tooltip::for_drive(label.clone(), drive.clone()))
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| this.show_folder(path.clone(), cx)))
            // The chevron's room: the label stays aligned with the main sidebar's, where
            // drives unfold their folders.
            .child(div().w_4().h_4().mr_1().flex_shrink_0())
            .child(
                div().w_4().h_4().mr_2().flex().items_center().justify_center().flex_shrink_0()
                    .child(svg().path(crate::ui::components::icons::drive_icon_path(drive)).size_4().text_color(theme.text)),
            )
            .child(div().flex_1().min_w_0().truncate().text_color(theme.text).font_weight(FontWeight::SEMIBOLD).child(label))
            .into_any_element()
    }

    pub(super) fn render_sidebar(&self, theme: OxydeTheme, cx: &mut Context<Self>) -> impl IntoElement {
        let mut rows: Vec<AnyElement> = self.places.iter().enumerate()
            .map(|(index, place)| self.render_place(index, place, theme, cx))
            .collect();
        if !self.drives.is_empty() {
            // The section line between quick access and the volumes.
            rows.push(div().w_full().h(ROW_HEIGHT).px_3().flex().items_center()
                .child(div().flex_1().h(rems(0.0625)).flex_shrink_0().bg(theme.border))
                .into_any_element());
        }
        rows.extend(self.drives.iter().enumerate().map(|(index, drive)| self.render_drive(index, drive, theme, cx)));
        let content_h = rem_pixels(ROW_HEIGHT).as_f32() * (self.places.len() + usize::from(!self.drives.is_empty())) as f32
            + rem_pixels(DRIVE_HEIGHT).as_f32() * self.drives.len() as f32;
        let scroll = self.sidebar_scroll.clone();
        let viewport_h = scroll.bounds().size.height.as_f32();
        let overflows = viewport_h > 0.0 && content_h > viewport_h + 1.0;
        let entity_id = cx.entity().entity_id();
        let dragging = self.scroll_drag.as_ref().is_some_and(|(track, _, _)| track.ends_with("file-chooser-sidebar"));
        div()
            .w(WIDTH)
            .h_full()
            .flex_none()
            .bg(theme.sidebar)
            .overflow_hidden()
            .relative()
            .child(
                div()
                    .absolute()
                    .top(LIST_TOP)
                    .bottom_0()
                    .left(rems(0.5))
                    // 12 px scrollbar track + 6 px clear margin when needed.
                    .right(if overflows { rems(1.125) } else { rems(0.5) })
                    .overflow_hidden()
                    .child(div().id("file-chooser-places").size_full().flex().flex_col().overflow_y_scroll().track_scroll(&scroll)
                        .on_scroll_wheel(move |_, _, cx| cx.notify(entity_id))
                        .children(rows.into_iter().map(|row| div().flex_none().child(row)))),
            )
            .child(
                div().absolute().top(LIST_TOP).bottom_0().right_0().w(rems(0.75))
                    .child(render_scrollbar_with_pointer(theme, cx.entity(), ScrollbarAxis::Vertical, viewport_h.max(1.0), if viewport_h > 0.0 { content_h } else { 1.0 },
                        -scroll.offset().y.as_f32(), dragging, "file-chooser-sidebar".into(), None,
                        move |this: &mut Self, track, viewport, content, _, cx| {
                            this.scroll_drag = Some((track, viewport, content));
                            cx.notify(entity_id);
                        })),
            )
    }
}
