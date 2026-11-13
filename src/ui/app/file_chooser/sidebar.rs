//! The places beside the entries, copies of the main sidebar's rows and list
//! (`sidebar/rows.rs`, `volume_row.rs`, `tree.rs`): quick access folders, the section
//! line, the mounted drives with their folder tree (`tree.rs`), then the volumes a
//! click mounts (`mount_volume`). Choosing needs no menus, drops or reordering.
use super::{FileChooserWindow, Place};
use crate::ui::paths::paths_match;
use crate::ui::components::sidebar::volume_row::name as volume_name;
use crate::ui::components::tooltip::hover::{HoverRegion, SharedTooltipExt};
use crate::ui::components::tooltip::Tooltip;
use crate::ui::elements::resize_separator::resize_separator;
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::scale::{logical, rem_pixels};
use crate::ui::theme::OxydeTheme;
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::models::DriveInfo;
use oxyde_lib::platform::UnmountedVolume;

/// Places, drives and volumes, at the height of the main sidebar's entries.
const ENTRY_HEIGHT: Rems = crate::ui::style::MENU_ITEM_HEIGHT;
/// Space above the first entry and below the last one, as the main sidebar's.
const LIST_PADDING: Rems = crate::ui::components::sidebar::LIST_PADDING;
/// One tooltip session for the whole list, as the main sidebar's.
pub(super) const REGION: HoverRegion = HoverRegion::Controls("file-chooser-places");

/// A place's label: the home folder's is translated.
fn place_name(place: &Place, i18n: &crate::ui::i18n::I18n) -> String {
    if place.label.is_empty() { i18n.text(Message::SidebarHome).to_string() } else { place.label.clone() }
}

impl FileChooserWindow {
    /// The sidebar's width now: as dragged, within what the window leaves it.
    pub(super) fn sidebar_display_width(&self) -> f32 {
        logical(super::arrange::sidebar_width(self.sidebar_width, self.window_width))
    }

    /// Mounts a volume of the sidebar and opens it, as the main window's
    /// `mount_volume`; the main window learns of the new drive as from its own.
    fn mount_volume(&mut self, id: String, cx: &mut Context<Self>) {
        if self.mounting_volume.is_some() {
            return;
        }
        self.mounting_volume = Some(id.clone());
        cx.notify();
        cx.spawn(async move |this, cx| {
            use oxyde_lib::platform::VolumeError;
            let result = tokio::task::spawn_blocking(move || {
                let volumes = oxyde_lib::platform::platform().volumes();
                let path = volumes.mount(&id)?;
                Ok::<_, VolumeError>((path, oxyde_lib::commands::system::get_drives(Some(false)), volumes.unmounted()))
            }).await;
            let _ = this.update(cx, |this, cx| {
                this.mounting_volume = None;
                let failure = match result {
                    Ok(Ok((path, drives, unmounted))) => {
                        let _ = this.owner.update(cx, |owner, cx| {
                            owner.drive_refresh_epoch = owner.drive_refresh_epoch.wrapping_add(1);
                            owner.state.unmounted_volumes = unmounted;
                            owner.apply_drive_snapshot(drives, cx);
                            owner.reconcile_watchers(cx);
                            cx.notify();
                        });
                        this.show_folder(path, cx);
                        None
                    }
                    // The user dismissed the authentication dialog.
                    Ok(Err(VolumeError::Cancelled)) => None,
                    Ok(Err(VolumeError::WindowsHibernated)) => Some((Message::DriveMountHibernated, String::new())),
                    Ok(Err(error)) => Some((Message::DriveMountFailed, error.to_string())),
                    Err(error) => Some((Message::DriveMountFailed, error.to_string())),
                };
                if let Some((message, detail)) = failure {
                    log::warn!("The file chooser cannot mount a volume: {detail}");
                    // Mount failures show where the main window shows them.
                    let _ = this.owner.update(cx, |owner, cx| owner.toast(super::super::ToastKind::Error, message, detail, 0, cx));
                }
                cx.notify();
            });
        }).detach();
    }

    fn render_place(&self, index: usize, place: &Place, owner: Option<&Entity<super::OxydeApp>>, theme: OxydeTheme, cx: &mut Context<Self>) -> AnyElement {
        let path = place.path.clone();
        let is_home = place.home;
        let is_sel = paths_match(&self.folder, &path);
        let hov_bg = if is_sel { theme.selection_hover } else { theme.hover };
        let is_dimmed = place.label.starts_with('.');
        let name = place_name(place, cx.i18n());
        div()
            .id(("file-chooser-place", index))
            .w_full()
            .h(ENTRY_HEIGHT)
            .px_3()
            .flex()
            .items_center()
            .when(is_dimmed, |d| d.opacity(0.75))
            .relative()
            .when(is_sel, |d| d.bg(theme.selected).child(div().absolute().left_0().top_0().bottom_0().w(rems(0.1875)).bg(theme.accent)))
            .hover(move |s| s.bg(hov_bg))
            .when_some(owner.cloned(), |d, owner| d.shared_tooltip(owner, REGION, format!("pinned:{path}"), cx, Tooltip::with_meta(name.clone(), path.clone())))
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| this.show_folder(path.clone(), cx)))
            .child(
                div().w_4().h_4().mr_2().flex().items_center().justify_center().flex_shrink_0()
                    .child(svg().path(if is_home { "icons/house.svg" } else { "icons/pin.svg" }).size_4()
                        .text_color(if is_home { theme.text } else { theme.accent })),
            )
            .child(div().flex_1().min_w_0().truncate().text_color(theme.text).child(name))
            .into_any_element()
    }

    fn render_drive(&self, index: usize, drive: &DriveInfo, owner: Option<&Entity<super::OxydeApp>>, theme: OxydeTheme, cx: &mut Context<Self>) -> AnyElement {
        let path = drive.path.clone();
        let is_sel = paths_match(&self.folder, &path);
        let hov_bg = if is_sel { theme.selection_hover } else { theme.hover };
        let label = crate::ui::format::drive_name(drive, cx.i18n());
        let expandable = self.tree_expandable(&path);
        div()
            .id(("file-chooser-drive", index))
            .w_full()
            .h(ENTRY_HEIGHT)
            .px_3()
            .flex()
            .items_center()
            .relative()
            .when(is_sel, |d| d.bg(theme.selected).child(div().absolute().left_0().top_0().bottom_0().w(rems(0.1875)).bg(theme.accent)))
            .hover(move |s| s.bg(hov_bg))
            .when_some(owner.cloned(), |d, owner| d.shared_tooltip(owner, REGION, format!("drive:{path}"), cx, Tooltip::for_drive(label.clone(), drive.clone())))
            // A press opens it, a double press unfolds it, as in the main sidebar.
            .on_mouse_down(MouseButton::Left, cx.listener({
                let path = path.clone();
                move |this, event: &MouseDownEvent, _, cx| {
                    if event.click_count == 2 && expandable {
                        this.toggle_tree(path.clone(), cx);
                    } else {
                        this.show_folder(path.clone(), cx);
                    }
                }
            }))
            .child(crate::ui::components::sidebar::folder_row::chevron(expandable, self.tree_expanded(&path), theme, {
                let this = cx.entity().downgrade();
                let path = path.clone();
                move |_, cx| { let _ = this.update(cx, |this, cx| this.toggle_tree(path.clone(), cx)); }
            }))
            .child(
                div().w_4().h_4().mr_2().flex().items_center().justify_center().flex_shrink_0()
                    .child(svg().path(crate::ui::components::icons::drive_icon_path(drive)).size_4().text_color(theme.text)),
            )
            .child(div().flex_1().min_w_0().truncate().text_color(theme.text).font_weight(FontWeight::SEMIBOLD).child(label))
            .into_any_element()
    }

    /// Unmounted volume row: dimmed after the mounted drives; a left click mounts it.
    fn render_volume(&self, volume: &UnmountedVolume, owner: Option<&Entity<super::OxydeApp>>, theme: OxydeTheme, cx: &mut Context<Self>) -> AnyElement {
        let label = volume_name(volume, cx.i18n());
        let size = crate::ui::format::format_size(volume.size, false, false, cx.i18n().language());
        let details = [volume.file_system.as_deref(), Some(size.as_str())].into_iter().flatten().collect::<Vec<_>>().join(" · ");
        let mounting = self.mounting_volume.as_deref() == Some(volume.id.as_str());
        let id = volume.id.clone();
        div()
            .id(SharedString::from(format!("file-chooser-volume-{}", volume.id)))
            .w_full()
            .h(ENTRY_HEIGHT)
            .px_3()
            .flex()
            .items_center()
            .hover(move |style| style.bg(theme.hover))
            .when_some(owner.cloned(), |row, owner| row.shared_tooltip(owner, REGION, format!("volume:{}", volume.id), cx,
                Tooltip::text(format!("{label}\n{details}"))))
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| this.mount_volume(id.clone(), cx)))
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

    pub(super) fn render_sidebar(&self, theme: OxydeTheme, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = self.tooltip_owner();
        let places = &self.places;
        let mut rows: Vec<AnyElement> = places.folders.iter().enumerate()
            .map(|(index, place)| self.render_place(index, place, owner.as_ref(), theme, cx))
            .collect();
        let volumes = !places.drives.is_empty() || !places.volumes.is_empty();
        if volumes {
            // The section line between quick access and the volumes.
            rows.push(crate::ui::elements::separator::horizontal(theme.border).px_3().into_any_element());
        }
        for (index, drive) in places.drives.iter().enumerate() {
            rows.push(self.render_drive(index, drive, owner.as_ref(), theme, cx));
            for folder in self.tree_folders(&drive.path) {
                rows.push(self.render_tree_folder(&folder, owner.as_ref(), theme, cx));
            }
        }
        rows.extend(places.volumes.iter().map(|volume| self.render_volume(volume, owner.as_ref(), theme, cx)));
        let content_h = self.sidebar_content_height();
        let scroll = self.sidebar_scroll.clone();
        let viewport_h = scroll.bounds().size.height.as_f32();
        let overflows = self.sidebar_overflows();
        let entity_id = cx.entity().entity_id();
        div()
            .w(px(self.sidebar_display_width()))
            .h_full()
            .flex_none()
            .bg(theme.sidebar)
            .on_scroll_wheel(cx.listener(|_, _, window, cx| {
                crate::ui::components::tooltip::hover::dismiss(window, cx);
                cx.notify();
            }))
            .overflow_hidden()
            .relative()
            .child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(rems(0.5))
                    // 12 px scrollbar track + 6 px clear margin when needed.
                    .right(if overflows { rems(1.125) } else { rems(0.5) })
                    .overflow_hidden()
                    .when_some(owner.clone(), |list, owner| list.child(crate::ui::components::tooltip::hover::region_container(owner, REGION, px(0.0), px(0.0), cx)))
                    .child(div().id("file-chooser-places").size_full().flex().flex_col().overflow_y_scroll().track_scroll(&scroll)
                        .on_scroll_wheel(move |_, _, cx| cx.notify(entity_id))
                        .child(div().flex_none().h(LIST_PADDING))
                        .children(rows.into_iter().map(|row| div().flex_none().child(row)))
                        .child(div().flex_none().h(LIST_PADDING))),
            )
            .child(
                div().absolute().top_0().bottom_0().right_0().w(rems(0.75))
                    .child(self.render_vertical_scrollbar(theme, super::scroll::SIDEBAR, scroll.clone(), viewport_h.max(1.0),
                        if viewport_h > 0.0 { content_h } else { 1.0 }, cx)),
            )
    }

    /// The height of the places' rows.
    fn sidebar_content_height(&self) -> f32 {
        let places = &self.places;
        let section = !places.drives.is_empty() || !places.volumes.is_empty();
        let folders: usize = places.drives.iter().map(|drive| self.tree_folders(&drive.path).len()).sum();
        rem_pixels(LIST_PADDING).as_f32() * 2.0
            + rem_pixels(ENTRY_HEIGHT).as_f32() * (places.folders.len() + places.drives.len() + places.volumes.len()) as f32
            + crate::ui::components::sidebar::folder_row::height() * folders as f32
            + if section { rem_pixels(crate::ui::style::MENU_SEPARATOR_HEIGHT).as_f32() } else { 0.0 }
    }

    /// Scrolls the places to center the drive or tree folder `path`, as the main
    /// sidebar reveals the folder it follows.
    pub(super) fn reveal_in_sidebar(&self, path: &str) {
        let places = &self.places;
        let (entry, folder_h) = (rem_pixels(ENTRY_HEIGHT).as_f32(), crate::ui::components::sidebar::folder_row::height());
        let mut top = rem_pixels(LIST_PADDING).as_f32() + entry * places.folders.len() as f32
            + if places.drives.is_empty() && places.volumes.is_empty() { 0.0 } else { rem_pixels(crate::ui::style::MENU_SEPARATOR_HEIGHT).as_f32() };
        let mut found = None;
        'drives: for drive in &places.drives {
            if paths_match(&drive.path, path) {
                found = Some((top, entry));
                break;
            }
            top += entry;
            for folder in self.tree_folders(&drive.path) {
                if paths_match(&folder.node.path, path) {
                    found = Some((top, folder_h));
                    break 'drives;
                }
                top += folder_h;
            }
        }
        let Some((top, height)) = found else { return };
        let scroll = &self.sidebar_scroll;
        let viewport = scroll.bounds().size.height.as_f32();
        let maximum = (self.sidebar_content_height() - viewport).max(0.0);
        scroll.set_offset(point(scroll.offset().x, px(-(top + height / 2.0 - viewport / 2.0).clamp(0.0, maximum))));
    }

    /// Whether the places overflow their list, which then shows a vertical scrollbar.
    fn sidebar_overflows(&self) -> bool {
        let viewport_h = self.sidebar_scroll.bounds().size.height.as_f32();
        viewport_h > 0.0 && self.sidebar_content_height() > viewport_h + 1.0
    }

    /// Double click on the edge: the width of the widest row, as the main sidebar's.
    fn fit_sidebar(&mut self, window: &Window, cx: &mut Context<Self>) {
        use crate::ui::components::sidebar::measure::{list_margins, LabelWidths, DEVICE_CHROME, PLACE_CHROME};
        let mut labels = LabelWidths::new(window);
        let i18n = cx.i18n();
        let (place_chrome, device_chrome) = (rem_pixels(PLACE_CHROME).as_f32(), rem_pixels(DEVICE_CHROME).as_f32());
        let mut widest = 0.0_f32;
        for place in &self.places.folders {
            widest = widest.max(labels.width(&place_name(place, i18n), false) + place_chrome);
        }
        for drive in &self.places.drives {
            widest = widest.max(labels.width(&crate::ui::format::drive_name(drive, i18n), true) + device_chrome);
        }
        for volume in &self.places.volumes {
            widest = widest.max(labels.width(&volume_name(volume, i18n), false) + device_chrome);
        }
        widest = widest.max(self.tree_width(&mut labels));
        if widest <= 0.0 {
            return;
        }
        let requested = (widest + list_margins(self.sidebar_overflows())) / logical(1.0);
        self.sidebar_width = super::arrange::sidebar_width(requested, self.window_width);
        self.options.sidebar = Some(self.sidebar_width);
    }

    /// The sidebar's edge, a copy of the main window's resize handle.
    pub(super) fn render_sidebar_edge(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let start = move |this: &mut Self, event: &MouseDownEvent, window: &Window, cx: &mut Context<Self>| {
            if event.click_count == 2 {
                this.fit_sidebar(window, cx);
            }
            this.resizing_sidebar = true;
            this.grab_offset = event.position.x.as_f32() - this.sidebar_display_width();
            cx.notify();
        };
        resize_separator("file-chooser-sidebar-edge", Axis::Horizontal)
            .dragging(self.resizing_sidebar)
            .on_press(cx.listener(move |this, event, window, cx| start(this, event, window, cx)))
    }

    /// The edge's drag, as the main window's `SidebarResize`.
    pub(super) fn resize_sidebar(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        if !event.dragging() {
            self.end_sidebar_resize(cx);
            return;
        }
        let requested = (event.position.x.as_f32() - self.grab_offset) / logical(1.0);
        let width = super::arrange::sidebar_width(requested, self.window_width);
        if width != self.sidebar_width {
            self.sidebar_width = width;
            self.options.sidebar = Some(width);
            cx.notify();
        }
    }

    pub(super) fn end_sidebar_resize(&mut self, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.resizing_sidebar) {
            cx.notify();
        }
    }
}
