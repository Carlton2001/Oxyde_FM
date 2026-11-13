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
    /// Height of the Settings page as laid out: Help keeps it, so the footer stays
    /// where it was rather than moving by the fraction the window rounded away.
    settings_height: std::rc::Rc<std::cell::Cell<Option<gpui::Pixels>>>,
    error: bool,
    focus: FocusHandle,
    pane: String,
    calendar: Option<(usize, chrono::NaiveDate)>,
    /// Keyboard of the date picker in its popup window.
    calendar_focus: FocusHandle,
    /// The two date fields, under which their picker opens.
    calendar_anchors: [Option<Bounds<Pixels>>; 2],
    controls: Vec<FocusHandle>,
    help_scroll: ScrollHandle,
    help_metrics: (f32, f32),
    help_drag: Option<(String, f32, f32)>,
}

pub struct AdvancedSearchWindow {
    owner: WeakEntity<OxydeApp>,
    advanced_search: Option<AdvancedSearchDialog>,
    context_menus: Vec<ContextMenuInstance>,
    /// Scroll state of the unit menu, so a second click on its control closes it.
    unit_menu: Option<UniformListScrollHandle>,
    /// When the native outside click dismissed the unit menu, before its control sees the click.
    unit_menu_dismissed: Option<std::time::Instant>,
    /// When the date picker of a field last closed, before its button sees the press.
    calendar_dismissed: Option<(usize, std::time::Instant)>,
    /// The unit button, under which its list opens at its width.
    unit_bounds: Option<Bounds<Pixels>>,
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
