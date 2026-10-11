mod calendar;
mod help;
mod render;
mod settings;
mod validation;
mod window;

pub use validation::parse_advanced_path;
pub(crate) use window::close_advanced_search_windows;

use super::{ContextMenuInstance, OxydeApp};
use crate::ui::components::input::TextInput;
use gpui::*;

pub struct AdvancedSearchDialog {
    inputs: Vec<Entity<TextInput>>,
    values: Vec<String>,
    flags: [bool; 6], // name regex, content regex, recursive, archives, case, accents
    unit: usize,
    help: bool,
    error: bool,
    focus: FocusHandle,
    pane: String,
    calendar: Option<(usize, chrono::NaiveDate)>,
    controls: Vec<FocusHandle>,
    help_scroll: ScrollHandle,
    help_metrics: (f32, f32),
    help_drag: Option<(String, f32, f32)>,
}

pub struct AdvancedSearchWindow {
    owner: WeakEntity<OxydeApp>,
    advanced_search: Option<AdvancedSearchDialog>,
    context_menus: Vec<ContextMenuInstance>,
    handle: WeakEntity<Self>,
    scrollbar_origins: std::collections::HashMap<String, Point<f32>>,
    grab_offset: f32,
    _subscriptions: Vec<Subscription>,
}

impl crate::ui::components::scrollbar::ScrollbarState for AdvancedSearchWindow {
    fn scrollbar_origins(&mut self) -> &mut std::collections::HashMap<String, Point<f32>> {
        &mut self.scrollbar_origins
    }
    fn set_grab_offset(&mut self, offset: f32) {
        self.grab_offset = offset;
    }
}
