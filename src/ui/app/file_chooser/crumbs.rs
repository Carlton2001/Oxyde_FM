//! The crumbs' state. Their motion and edge fades are the main navigation bar's
//! (`top_bar/breadcrumb_glide.rs`, `top_bar/breadcrumb_fades.rs`).
use crate::ui::components::top_bar::breadcrumb_glide::Glide;
use gpui::*;
use oxyde_lib::models::FileEntry;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[derive(Default)]
pub(super) struct Crumbs {
    pub(super) scroll: ScrollHandle,
    /// The path the crumbs were last laid out for.
    pub(super) path: RefCell<String>,
    pub(super) glide: Rc<Glide>,
    pub(super) viewport: Rc<Cell<f32>>,
    /// A chevron's open menu: the folder it lists, the chevron's index, its folders.
    pub(super) menu: Option<(String, usize, Vec<FileEntry>)>,
    /// The chevron whose folder is being listed, before its menu opens.
    pub(super) request: Option<(String, usize)>,
    pub(super) menu_scroll: UniformListScrollHandle,
}
