mod card;
mod footer;
mod panel;
mod view;

pub use footer::render_operations_footer;
pub use panel::render_operations_panel;

use crate::ui::app::OxydeApp;
use crate::ui::scale::rem_pixels;
use gpui::*;
use oxyde_lib::systems::file_ops::OpStatus;

pub(crate) const PANEL_WIDTH: Rems = rems(22.5);
pub(crate) fn panel_width() -> f32 { rem_pixels(PANEL_WIDTH).as_f32() }
const PANEL_MAX_HEIGHT: Rems = rems(26.25);
fn panel_max_height() -> f32 { rem_pixels(PANEL_MAX_HEIGHT).as_f32() }

pub struct OperationsPanelView {
    app: WeakEntity<OxydeApp>,
    _app_subscription: Subscription,
    position: Option<Point<Pixels>>,
    bounds: Option<Bounds<Pixels>>,
    drag_offset: Option<Point<Pixels>>,
    last_drag_update: Option<std::time::Instant>,
}

fn is_final(status: &OpStatus) -> bool {
    matches!(
        status,
        OpStatus::Completed | OpStatus::Cancelled | OpStatus::Error(_)
    )
}
