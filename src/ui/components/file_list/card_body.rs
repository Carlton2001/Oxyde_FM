//! What a card shows, inside the frame `grid_card` draws and wires: the grid's icon
//! above the name and the type, or a tile's icon beside the name, the type and a
//! detail line. Measures come from `style::cards`, which the hit tests also read.
use super::cloud_status::sized_cloud_status_icon;
use super::presentation::{file_type_display, network_category, network_entry, selection_checkbox, size_loading, SELECTION_CHECKBOX_TARGET_SIZE};
use super::thumbnails::render_grid_thumbnail;
use crate::ui::app::OxydeApp;
use crate::ui::components::icons::render_file_icon;
use crate::ui::elements::checkbox::CHECKBOX_SIZE;
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::style::{CardLayout, CardSpec, TabularFigures, CARD_BORDER, CARD_DETAIL_HEIGHT, CARD_NAME_HEIGHT, GRID_CARD_GAP, GRID_CARD_PADDING_X, GRID_CARD_PADDING_Y, TILE_PADDING_X, TILE_PADDING_Y};
use crate::ui::theme::OxydeTheme;
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::models::{CloudStatus, FileEntry};

/// The radius of `rounded_md`, which the card and its name's overlay share.
const CARD_RADIUS: Rems = rems(0.375);
/// Inset of the checkbox and cloud badge, measured from their card corners.
pub(super) const GRID_CARD_CONTROL_INSET: Rems = rems(0.375);
/// Lines a long name takes in its overlay before it is cut.
const NAME_OVERLAY_LINES: usize = 4;

pub(super) struct CardBody<'a> {
    pub(super) file: &'a FileEntry,
    pub(super) path: String,
    pub(super) name: String,
    pub(super) is_dir: bool,
    pub(super) has_subdirectories: bool,
    pub(super) is_selected: bool,
    pub(super) h: Entity<OxydeApp>,
    pub(super) pid: String,
    pub(super) icon: crate::ui::components::icons::IconType,
    pub(super) editing_view: Option<Entity<crate::ui::components::input::TextInput>>,
    pub(super) theme: OxydeTheme,
    pub(super) loading_size: bool,
    pub(super) provisional_size: bool,
    pub(super) show_grid_thumbnails: bool,
    pub(super) use_system_icons: bool,
    pub(super) show_checkboxes: bool,
    /// The whole name shows over the cards below: the card is hovered or selected alone.
    pub(super) expanded: bool,
    /// The card's border and fills, which the name's overlay continues.
    pub(super) border: Hsla,
    pub(super) fill: Hsla,
    pub(super) pane_fill: Hsla,
}

impl CardBody<'_> {
    fn dimmed(&self) -> bool {
        self.file.is_hidden || self.file.is_system
    }

    fn kind(&self, cx: &App) -> String {
        if network_entry(self.file) {
            network_category(self.file, cx.i18n())
        } else if self.file.is_dir {
            cx.i18n().text(Message::FileTypeFolder).to_uppercase()
        } else {
            file_type_display(self.file, cx.i18n())
        }
    }

    /// The thumbnail, else the icon, centered; dimmed while the folder's size is measured.
    fn media(&self, spec: CardSpec) -> Div {
        let picture = if self.show_grid_thumbnails && !self.is_dir && super::thumbnails::supports_thumbnail(self.file) {
            render_grid_thumbnail(self.file, &self.pid, self.icon, self.theme, self.use_system_icons, spec).into_any_element()
        } else {
            render_file_icon(self.path.clone(), self.icon, spec.icon, &self.theme, self.use_system_icons, true)
        };
        div()
            .relative()
            .when(self.dimmed(), |d| d.opacity(0.75))
            .child(div().absolute().inset_0().flex().items_center().justify_center()
                .when(self.loading_size, |icon| icon.opacity(0.3))
                .child(picture))
            .when(self.loading_size, |area| area.child(
                div().absolute().inset_0().flex().items_center().justify_center()
                    .child(size_loading(&self.path, self.theme.accent)),
            ))
    }

    fn chevron(&self) -> Svg {
        svg().path("icons/chevron-right.svg").size_3().text_color(self.theme.text).opacity(0.5)
    }

    /// The name, or the rename field in its place; a click on the selected name renames.
    fn name(&self, shown: bool) -> AnyElement {
        if let Some(editing_view) = self.editing_view.clone() {
            return editing_view.into_any_element();
        }
        let h = self.h.clone();
        let path = self.path.clone();
        let is_dir = self.is_dir;
        div()
            // Within the card's own identifier, which already names the path.
            .id("rename-name")
            .font_weight(FontWeight::MEDIUM)
            .when(shown, |d| d.child(self.name.clone()))
            // Its overlay shows the name: this line keeps the place and the click.
            .when(!shown, |d| d.w_full().h_full())
            .on_click(move |event, window, cx| {
                h.update(cx, |this, cx| this.handle_slow_rename_click(path.clone(), is_dir, event, window, cx));
            })
            .into_any_element()
    }

    fn kind_line(&self, kind: String) -> Stateful<Div> {
        div()
            .id("size-provisional-grid")
            .w_full()
            .h(CARD_DETAIL_HEIGHT)
            .line_height(CARD_DETAIL_HEIGHT)
            .text_size(rems(0.625))
            .text_color(if self.provisional_size { self.theme.accent } else { self.theme.text_muted })
            .text_ellipsis()
            .overflow_hidden()
            .when(self.dimmed(), |d| d.opacity(0.75))
            .child(kind)
    }
}

/// The grid card: its icon area, the name on one line, cut in the middle so the
/// extension stays, and the type. Expanded, the whole name and the type show in an
/// overlay that runs past the card's bottom in its colors.
pub(super) fn grid(card: Stateful<Div>, body: CardBody<'_>, layout: CardLayout, cx: &App) -> Stateful<Div> {
    let spec = layout.spec();
    let kind = body.kind(cx);
    let expanded = body.expanded && body.editing_view.is_none();
    let name_line = div()
        .w_full()
        .h(CARD_NAME_HEIGHT)
        .line_height(CARD_NAME_HEIGHT)
        .text_center()
        .text_size(rems(0.75))
        .text_color(body.theme.text)
        .text_ellipsis_middle()
        .overflow_hidden()
        .when(body.dimmed(), |d| d.opacity(0.75))
        .child(body.name(!expanded));
    let card = card.px(GRID_CARD_PADDING_X)
        .py(GRID_CARD_PADDING_Y)
        .flex()
        .flex_col()
        .items_center()
        .gap(GRID_CARD_GAP)
        .child(
            body.media(spec)
                .w_full()
                .h(spec.media)
                .flex_none()
                .when_some(body.file.cloud_status, |area, status| area.child(
                    // The media area already starts inside the card padding.
                    div().absolute()
                        .left(rems(GRID_CARD_CONTROL_INSET.0 - GRID_CARD_PADDING_X.0))
                        .bottom(rems(GRID_CARD_CONTROL_INSET.0 - GRID_CARD_PADDING_Y.0))
                        .size(SELECTION_CHECKBOX_TARGET_SIZE)
                        .flex().items_center().justify_center()
                        .child(sized_cloud_status_icon(status, &body.theme, CHECKBOX_SIZE)
                            .when(status != CloudStatus::AlwaysAvailable, |icon| {
                                icon.rounded_full().bg(body.theme.badge_surface)
                            })),
                ))
                .when(body.has_subdirectories, |area| area.child(
                    div().absolute().right_0().top_0().h_full().flex().items_center().child(body.chevron()),
                )),
        )
        .child(name_line);
    if expanded {
        card.child(div().w_full().h(CARD_DETAIL_HEIGHT))
            .child(crate::ui::elements::overlay::clipped_overlay(name_overlay(&body, spec, kind)))
    } else {
        card.child(body.kind_line(kind).text_center())
    }
}

/// The name wrapped over several lines and the type, where the card shows them. The
/// fill only starts near the card's bottom, so the card's own fills (size bar
/// included) stay visible under the text and the overlay continues them below.
fn name_overlay(body: &CardBody<'_>, spec: CardSpec, kind: String) -> impl IntoElement {
    let content = CARD_NAME_HEIGHT.0 + GRID_CARD_GAP.0 + CARD_DETAIL_HEIGHT.0 + GRID_CARD_PADDING_Y.0;
    let fill = div()
        .absolute()
        .top(rems(content - CARD_RADIUS.0))
        .left_0()
        .right_0()
        .bottom(rems(-CARD_BORDER.0))
        .rounded_b(CARD_RADIUS)
        .bg(body.pane_fill)
        .child(div().size_full().rounded_b(CARD_RADIUS).bg(body.fill)
            .border_l(CARD_BORDER).border_r(CARD_BORDER).border_b(CARD_BORDER).border_color(body.border));
    div()
        .absolute()
        .top(rems(GRID_CARD_PADDING_Y.0 + spec.media.0 + GRID_CARD_GAP.0))
        .left(rems(-CARD_BORDER.0))
        .right(rems(-CARD_BORDER.0))
        .px(rems(CARD_BORDER.0 + GRID_CARD_PADDING_X.0))
        .pb(GRID_CARD_PADDING_Y)
        .flex()
        .flex_col()
        .items_center()
        .gap(GRID_CARD_GAP)
        .child(fill)
        .child(div()
            .relative()
            .w_full()
            .line_height(CARD_NAME_HEIGHT)
            .text_center()
            .text_size(rems(0.75))
            .font_weight(FontWeight::MEDIUM)
            .text_color(body.theme.text)
            .line_clamp(NAME_OVERLAY_LINES)
            // GPUI cuts a middle ellipsis to a single line, even with several allowed.
            .text_ellipsis()
            .when(body.dimmed(), |d| d.opacity(0.75))
            .child(body.name.clone()))
        .child(body.kind_line(kind).relative().text_center())
}

/// The tile: the checkbox, the icon, then the name, the size with the modification
/// date and the type, and the chevron of a folder holding folders.
pub(super) fn tile(card: Stateful<Div>, body: CardBody<'_>, layout: CardLayout, cx: &App) -> Stateful<Div> {
    let spec = layout.spec();
    let kind = body.kind(cx);
    let status = body.file.cloud_status.filter(|_| body.editing_view.is_none());
    let name_line = div()
        .w_full()
        .h(CARD_NAME_HEIGHT)
        .line_height(CARD_NAME_HEIGHT)
        .text_size(rems(0.8125))
        .text_color(body.theme.text)
        .text_ellipsis_middle()
        .overflow_hidden()
        .when(body.dimmed(), |d| d.opacity(0.75))
        .when(status.is_some(), |line| line.flex().items_center().gap(rems(0.375)))
        .child(if status.is_some() {
            div().min_w_0().overflow_hidden().child(body.name(true)).into_any_element()
        } else {
            body.name(true)
        })
        .when_some(status, |line, status| line.child(super::cloud_status_icon(status, &body.theme)));
    card.px(TILE_PADDING_X)
        .py(TILE_PADDING_Y)
        .flex()
        .flex_row()
        .items_center()
        .gap(rems(0.625))
        .when(body.show_checkboxes, |card| card.child(
            selection_checkbox(body.is_selected, body.path.clone(), body.pid.clone(), body.h.clone(), body.theme),
        ))
        .child(body.media(spec).size(spec.media).flex_none())
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .justify_center()
                .child(name_line)
                .child(tile_details(&body, cx))
                .child(body.kind_line(kind).text_size(rems(0.6875))),
        )
        .when(body.has_subdirectories, |card| card.child(div().flex_none().child(body.chevron())))
}

/// The size, measured or being measured for a folder, then the modification date;
/// a network entry shows where it is.
fn tile_details(body: &CardBody<'_>, cx: &App) -> Div {
    let file = body.file;
    let line = div()
        .w_full()
        .h(CARD_DETAIL_HEIGHT)
        .flex()
        .items_center()
        .gap(rems(0.25))
        .overflow_hidden()
        .text_size(rems(0.6875))
        .line_height(CARD_DETAIL_HEIGHT)
        .text_color(body.theme.text_muted)
        .tabular_figures()
        .when(body.dimmed(), |d| d.opacity(0.75));
    if network_entry(file) {
        let location = file.network_details.as_ref().map(|details| details.location.clone()).unwrap_or_default();
        return line.child(div().min_w_0().truncate().child(location));
    }
    let language = cx.i18n().language();
    let size: Option<AnyElement> = if body.loading_size {
        Some(size_loading(&body.path, body.theme.accent))
    } else if file.is_dir && !file.is_calculated {
        None
    } else if file.is_dir && file.size == 0 {
        Some(div().child(cx.i18n().text(Message::FolderSizeEmpty)).into_any_element())
    } else {
        Some(div().flex_none()
            .when(body.provisional_size, |d| d.text_color(body.theme.accent))
            .child(crate::ui::format::format_size(file.size, file.is_dir, file.is_calculated, language))
            .into_any_element())
    };
    let date = crate::ui::format::format_date(file.modified, crate::ui::format::date_format(cx));
    line.when_some(size, |line, size| line.child(size).child(div().flex_none().child("•")))
        .child(div().min_w_0().truncate().child(date))
}
