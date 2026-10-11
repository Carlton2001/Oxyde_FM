mod lifecycle;
mod results;
mod search;
mod sources;
mod window;

pub(crate) use lifecycle::close_duplicate_windows;

use super::OxydeApp;
use crate::ui::components::input::TextInput;
use gpui::{AnyView, Entity, FocusHandle, ScrollHandle, Subscription, Task, WeakEntity};
use oxyde_lib::commands::duplicates::{cancel_find_duplicates, DuplicateGroup, DuplicateSearchState};
use std::{collections::HashSet, sync::Arc};

pub struct DuplicateWindow {
    owner: WeakEntity<OxydeApp>,
    locations: Vec<(String, String, String)>,
    location_icons: Vec<&'static str>,
    selected: HashSet<String>,
    options: [bool; 3],
    groups: Arc<Vec<DuplicateGroup>>,
    collapsed: HashSet<usize>,
    filter: Entity<TextInput>,
    query: String,
    filtered: Vec<usize>,
    results_scroll: ScrollHandle,
    sources_scroll: ScrollHandle,
    running: bool,
    searched: bool,
    error: Option<String>,
    cancellation: Arc<DuplicateSearchState>,
    limit: usize,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
    resize_task: Option<Task<()>>,
    filter_task: Option<Task<()>>,
    filtering: bool,
    hovered_result: Option<String>,
    result_tooltip: Option<(String, AnyView)>,
    tooltip_task: Option<Task<()>>,
}

impl Drop for DuplicateWindow {
    fn drop(&mut self) {
        cancel_find_duplicates(&self.cancellation);
    }
}
