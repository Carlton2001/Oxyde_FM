//! The operations window, laid out as a file manager: the operations listed beside
//! the selected one's progress and its items in a details list. The compact progress
//! notifications and the title bar button open it.
mod compact;
mod detail;
mod footer;
mod items;
mod lifecycle;
mod list;
mod view;

pub(crate) use compact::render_operation_progress;
pub use footer::render_operations_footer;
pub(crate) use lifecycle::{close_operations_windows, open_operations_window};

use crate::ui::app::OxydeApp;
use gpui::*;
use oxyde_lib::systems::file_ops::OpStatus;

pub struct OperationsPanelView {
    app: WeakEntity<OxydeApp>,
    _app_subscription: Subscription,
    focus: FocusHandle,
    /// The operation shown on the right; the newest running one when none was chosen.
    selected: Option<String>,
    list_scroll: ScrollHandle,
    /// The list's width as dragged, in rems, saved in the preferences; its default
    /// until then.
    list_width: Option<f32>,
    /// Pixels per rem at the last layout, which converts the dragged width.
    rem_px: f32,
    resizing_list: bool,
    list_edge_hovered: bool,
    items: items::Items,
    scrollbar_origins: std::collections::HashMap<String, Point<f32>>,
    grab_offset: f32,
    /// The dragged thumb's track, viewport and content heights.
    scroll_drag: Option<(String, f32, f32)>,
}

fn is_final(status: &OpStatus) -> bool {
    matches!(
        status,
        OpStatus::Completed | OpStatus::Cancelled | OpStatus::Error(_)
    )
}
