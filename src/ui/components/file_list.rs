mod details;
mod details_row;
mod filters;
mod grid;
mod grid_scroll;
mod grid_card;
mod headers;
mod presentation;
mod selection_button;
mod thumbnails;

pub use filters::{get_date_filter_items, get_size_filter_items, get_type_filter_items};
pub(crate) use presentation::file_type_label;
pub(crate) use thumbnails::invalidate_thumbnail;

use crate::ui::app::OxydeApp;
use crate::ui::i18n::I18nContext;
use crate::ui::scale::rem_pixels;
use crate::ui::theme::ThemeContext;
use crate::ui::workspace::ViewMode;
use gpui::prelude::*;
use gpui::*;

use crate::ui::style::STATUS_BAR_HEIGHT;
fn status_bar_height() -> f32 { rem_pixels(STATUS_BAR_HEIGHT).as_f32() }

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
