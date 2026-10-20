mod details;
mod details_row;
mod filters;
mod grid;
mod grid_scroll;
mod grid_card;
mod headers;
mod presentation;
mod group_header;
mod selection_button;
mod scroll_top;
mod thumbnails;

pub use filters::{get_date_filter_items, get_size_filter_items, get_type_filter_items};
pub(crate) use presentation::{file_type_display, file_type_label, network_entry};
pub(crate) use thumbnails::{invalidate_thumbnail, clear_grid_thumbnails};

use crate::ui::app::OxydeApp;
use crate::ui::i18n::I18nContext;
use crate::ui::scale::rem_pixels;
use crate::ui::theme::ThemeContext;
use crate::ui::workspace::ViewMode;
use gpui::prelude::*;
use gpui::*;

use crate::ui::style::STATUS_BAR_HEIGHT;
fn status_bar_height() -> f32 { rem_pixels(STATUS_BAR_HEIGHT).as_f32() }

fn protected_folder_message(message: String, theme: &crate::ui::theme::OxydeTheme) -> Div {
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(rems(1.0))
        .px(rems(1.0))
        .text_color(theme.text_muted)
        .child(svg().path("icons/ban.svg").size(rems(3.5)).text_color(theme.text_muted))
        .child(div().text_size(rems(0.8125)).text_align(TextAlign::Center).italic().child(message))
}

/// Vertical rail placement (top, height) matching the measured list viewport,
/// relative to the container recorded in `list_bounds`.
fn measured_rail(view: &OxydeApp, pane_id: &str, scroll: &ScrollHandle) -> Option<(f32, f32)> {
    let list = scroll.bounds();
    let container = view.list_bounds.get(pane_id)?;
    let top = list.origin.y.as_f32() - container.origin.y;
    // Clip to the visible container: the list may overflow below its padded parent.
    let height = list.size.height.as_f32().min(container.size.height - top);
    (height > 0.0).then_some((top, height))
}

pub fn render_file_list(
    view: &OxydeApp,
    pane_id: String,
    handle: Entity<OxydeApp>,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let p_id_cap = pane_id.clone();
    let theme = *cx.theme();
    let language = cx.i18n().language();
    let pane = view.state.layout.find_pane(&pane_id).expect("pane exists");
    if pane.active_tab().view_mode == ViewMode::Grid {
        return grid::render_grid_file_list(view, pane_id, handle, cx).into_any_element();
    }
    details::render_details_file_list(view, pane_id, handle, p_id_cap, theme, language, pane, cx)
}
