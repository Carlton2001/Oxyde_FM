mod card;
mod compact;
mod footer;
mod lifecycle;
mod panel;
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
}

fn is_final(status: &OpStatus) -> bool {
    matches!(
        status,
        OpStatus::Completed | OpStatus::Cancelled | OpStatus::Error(_)
    )
}
