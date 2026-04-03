use gpui::*;
use gpui::prelude::*;
use crate::ui::theme::{PixelsExt, ThemeContext};
use crate::ui::workspace::WorkspaceState;
use oxyde_lib::models::FileEntry;
use crate::ui::components::sidebar::render_sidebar;
use crate::ui::components::top_bar::render_top_bar;
use crate::ui::components::file_list::render_file_list;
use crate::ui::components::tabs::{TabBar, Tab, TabPosition, TabDragPreview};
use crate::ui::style::{TNUM, ROW_HEIGHT};
use oxyde_lib::commands::io::list_dir;

actions!(oxyde, [Ouvrir, Copier, Couper, Coller, Renommer, Supprimer]);



#[derive(Debug, PartialEq, Clone)]
pub enum DragTarget {
    SidebarV { viewport_h: f32, content_h: f32, track_id: String },
    SidebarH { viewport_w: f32, content_w: f32, track_id: String },
    SidebarResize,
    FileListV { pane_id: String, viewport_h: f32, content_h: f32, track_id: String },
    FileListH { pane_id: String, viewport_w: f32, content_w: f32, track_id: String },
    BreadcrumbV { viewport_h: f32, content_h: f32, track_id: String },
    ContextMenuV { viewport_h: f32, content_h: f32, track_id: String },
    PaneResize { split_id: String, child_idx: usize, initial_weights: Vec<f32> },
    SelectionRectangle { pane_id: String, start_pos: Point<Pixels>, current_pos: Point<Pixels>, modifiers: Modifiers, initial_selection: std::collections::HashSet<String> },
}

#[derive(Clone)]
pub enum ContextMenuHeader {
    TypeSelectAll { pane_id: String },
    SizeSelectAll { pane_id: String },
    DateSelectAll { pane_id: String },
}

#[derive(Clone)]
pub enum ContextMenuContent {
    Items(Vec<crate::ui::components::dropdown::DropdownItem>, Option<ContextMenuHeader>),
    NameFilter { input: Entity<crate::ui::components::input::TextInput> },
}

pub struct OxydeApp {
    pub state: WorkspaceState,
    pub handle: Option<Entity<Self>>,
    pub(crate) sidebar_scroll: UniformListScrollHandle,
    pub(crate) sidebar_scroll_h: ScrollHandle,
    pub(crate) breadcrumb_scroll: UniformListScrollHandle,
    pub drag_target: Option<DragTarget>,
    pub drop_target_idx: Option<usize>,
    pub drop_target_pane: Option<String>,
    pub highlight_right_zone: bool,
    pub highlight_left_zone: bool,
    pub highlight_top_zone: bool,
    pub highlight_bottom_zone: bool,
    pub highlight_split_pane: Option<String>,
    pub grab_offset: f32,
    pub sidebar_width: f32,
    pub sidebar_content_width: f32,
    pub list_height: f32,
    pub list_width: f32,
    pub split_sizes: std::collections::HashMap<String, f32>,
    pub pane_sizes: std::collections::HashMap<String, Size<f32>>,
    pub pane_origins: std::collections::HashMap<String, Point<f32>>,
    pub list_bounds: std::collections::HashMap<String, Bounds<f32>>,
    pub scrollbar_origins: std::collections::HashMap<String, Point<f32>>,
    pub breadcrumb_menu: Option<(String, String, usize, Vec<FileEntry>)>, // (pane_id, path, index, dirs)
    pub context_menu: Option<(Point<Pixels>, ContextMenuContent)>,
    pub(crate) context_menu_scroll: UniformListScrollHandle,
    pub dragging_tab: Option<(String, usize)>,
}

impl OxydeApp {
    pub fn handle_selection_drag(&mut self, pane_id: String, current_pos: Point<Pixels>, cx: &mut Context<Self>) {
        if let Some(pane) = self.state.find_pane_mut(&pane_id) {
            let tab = pane.active_tab_mut();
            let bounds = self.list_bounds.get(&pane_id).cloned().unwrap_or(Bounds { origin: Point { x: 0.0, y: 0.0 }, size: Size { width: 1000.0, height: 800.0 } });
            let scroll_y = tab.list_scroll.0.borrow().base_handle.offset().y.as_f32();
            
            // --- Auto-Scroll (Intuitive & Proportional) ---
            let list_top = bounds.origin.y;
            let list_bottom = bounds.origin.y + bounds.size.height;
            let mut new_scroll_y = scroll_y;
            let mut scrolled = false;

            if current_pos.y.as_f32() < list_top {
                let dist = (list_top - current_pos.y.as_f32()).max(1.0);
                let speed = (dist / 10.0).powf(1.2).min(40.0); 
                new_scroll_y += speed;
            } else if current_pos.y.as_f32() > list_bottom {
                let dist = (current_pos.y.as_f32() - list_bottom).max(1.0);
                let speed = (dist / 10.0).powf(1.2).min(40.0);
                new_scroll_y -= speed;
            }
            
            if new_scroll_y != scroll_y {
                let count = tab.filtered_files().len() as f32;
                let visible_list_h = (bounds.size.height - ROW_HEIGHT).max(1.0);
                let content_h = (count * ROW_HEIGHT).max(1.0);
                let max_scroll = (content_h - visible_list_h).max(0.0);
                
                new_scroll_y = new_scroll_y.clamp(-max_scroll, 0.0);
                if new_scroll_y != scroll_y {
                    tab.list_scroll.0.borrow().base_handle.set_offset(Point::new(px(0.0), px(new_scroll_y)));
                    scrolled = true;
                }
            }
            let scroll_y = new_scroll_y;

            // --- Selection Logic ---
            if let Some(DragTarget::SelectionRectangle { start_pos, modifiers, initial_selection, .. }) = &self.drag_target {
                let start_pos = *start_pos;
                let modifiers = *modifiers;
                let initial_selection_cap = initial_selection.clone();

                let rect_y_start = start_pos.y.as_f32().min(current_pos.y.as_f32());
                let rect_y_end = start_pos.y.as_f32().max(current_pos.y.as_f32());
                
                let files = tab.filtered_files();
                let list_origin_y = bounds.origin.y + ROW_HEIGHT;
                
                let start_idx_f = (rect_y_start - list_origin_y - scroll_y) / ROW_HEIGHT;
                let end_idx_f = (rect_y_end - list_origin_y - scroll_y) / ROW_HEIGHT;
                
                let start_idx = (start_idx_f.floor() as i32).max(0) as usize;
                let end_idx = (end_idx_f.ceil() as i32).max(0) as usize;
                
                let files_len = files.len();
                let start_idx = start_idx.min(files_len);
                let end_idx = end_idx.min(files_len);
                
                let current_rect_paths: std::collections::HashSet<String> = if start_idx < end_idx {
                    files[start_idx..end_idx].iter().map(|f| f.path.clone()).collect()
                } else {
                    std::collections::HashSet::new()
                };

                if modifiers.control {
                    tab.selected_paths.clear();
                    for path in initial_selection_cap.iter() {
                        if !current_rect_paths.contains(path) { tab.selected_paths.insert(path.clone()); }
                    }
                    for path in current_rect_paths.iter() {
                        if !initial_selection_cap.contains(path) { tab.selected_paths.insert(path.clone()); }
                    }
                } else if modifiers.shift {
                    for path in initial_selection_cap.iter() { tab.selected_paths.insert(path.clone()); }
                    for path in current_rect_paths.iter() { tab.selected_paths.insert(path.clone()); }
                } else {
                    tab.selected_paths = current_rect_paths;
                    tab.selection_anchor = None;
                }
            }
            
            if scrolled {
                cx.notify();
            }
        }
    }

    pub fn empty(_cx: &mut Context<Self>) -> Self {
        Self {
            state: WorkspaceState::new(),
            handle: None,
            sidebar_scroll: UniformListScrollHandle::new(),
            sidebar_scroll_h: ScrollHandle::new(),
            breadcrumb_scroll: UniformListScrollHandle::new(),
            drag_target: None,
            drop_target_idx: None,
            drop_target_pane: None,
            highlight_right_zone: false,
            highlight_left_zone: false,
            highlight_top_zone: false,
            highlight_bottom_zone: false,
            highlight_split_pane: None,
            grab_offset: 0.0,
            sidebar_width: 260.0,
            sidebar_content_width: 260.0,
            list_height: 800.0,
            list_width: 1000.0,
            split_sizes: std::collections::HashMap::new(),
            pane_sizes: std::collections::HashMap::new(),
            pane_origins: std::collections::HashMap::new(),
            list_bounds: std::collections::HashMap::new(),
            scrollbar_origins: std::collections::HashMap::new(),
            breadcrumb_menu: None,
            context_menu: None,
            context_menu_scroll: UniformListScrollHandle::new(),
            dragging_tab: None
        }
    }

    pub fn navigate_to(&mut self, pane_id: &str, path: String, cx: &mut Context<Self>) {
        if let Some(pane) = self.state.find_pane_mut(pane_id) {
            let active_tab = pane.active_tab_mut();
            if path == active_tab.path {
                return;
            }
            let old_path = active_tab.path.clone();
            active_tab.back_stack.push(old_path);
            active_tab.forward_stack.clear();
            self.load_path_sync(pane_id, path, cx);
        }
    }

    pub fn go_back(&mut self, pane_id: &str, cx: &mut Context<Self>) {
        if let Some(pane) = self.state.find_pane_mut(pane_id) {
            let active_tab = pane.active_tab_mut();
            if let Some(path) = active_tab.back_stack.pop() {
                let current = active_tab.path.clone();
                active_tab.forward_stack.push(current);
                self.load_path_sync(pane_id, path, cx);
            }
        }
    }

    pub fn go_forward(&mut self, pane_id: &str, cx: &mut Context<Self>) {
        if let Some(pane) = self.state.find_pane_mut(pane_id) {
            let active_tab = pane.active_tab_mut();
            if let Some(path) = active_tab.forward_stack.pop() {
                let current = active_tab.path.clone();
                active_tab.back_stack.push(current);
                self.load_path_sync(pane_id, path, cx);
            }
        }
    }

    pub fn add_tab(&mut self, pane_id: &str, path: String, activate: bool, cx: &mut Context<Self>) {
        self.state.add_tab(pane_id, path, activate);
        cx.notify();
    }

    pub fn switch_tab(&mut self, pane_id: &str, tab_idx: usize, cx: &mut Context<Self>) {
        if let Some(pane) = self.state.find_pane_mut(pane_id) {
            pane.active_tab_index = tab_idx;
            self.state.active_pane_id = pane_id.to_string();
            cx.notify();
        }
    }

    pub fn clamp_scrolls(&mut self) {
        // Clamp Sidebar horizontal scroll
        let sidebar_viewport_w = self.sidebar_width;
        let sidebar_content_w = self.sidebar_content_width;
        let sidebar_max_offset_x = (sidebar_content_w - sidebar_viewport_w).max(0.0);
        let current_sidebar_h_offset = self.sidebar_scroll_h.offset().x.as_f32().abs();
        
        if current_sidebar_h_offset > sidebar_max_offset_x {
             let new_x = (-sidebar_max_offset_x).min(0.0);
             self.sidebar_scroll_h.set_offset(Point::new(px(new_x), self.sidebar_scroll_h.offset().y));
        }

        // Clamp FileList horizontal scrolls for all panes
        let p_sizes = &self.pane_sizes;
        let l_w = self.list_width;
        let l_h = self.list_height;

        self.state.layout.visit_panes_mut(&mut |pane| {
             let pane_id = pane.id.clone();
             let active_tab = pane.active_tab_mut();
             let pane_size = p_sizes.get(&pane_id).cloned().unwrap_or(Size { width: l_w, height: l_h });
             
             // Constants matching file_list.rs
             let type_w = 100.0;
             let size_w = 100.0;
             let date_w = 180.0;
             let fixed_w = type_w + size_w + date_w;
             let name_w = (pane_size.width - fixed_w).max(80.0);
             let total_w = name_w + fixed_w;
             
             let max_h_offset = (total_w - pane_size.width).max(0.0);
             let current_h_offset = active_tab.list_scroll_h.offset().x.as_f32().abs();
             
             if current_h_offset > max_h_offset {
                  let new_x = (-max_h_offset).min(0.0);
                  active_tab.list_scroll_h.set_offset(Point::new(px(new_x), active_tab.list_scroll_h.offset().y));
             }
        });

        // Clamp Breadcrumb scroll
        if let Some((_, _, _, dirs)) = &self.breadcrumb_menu {
             let item_count = dirs.len();
             let content_h = item_count as f32 * 28.0;
             let viewport_h = content_h.min(360.0);
             let max_v_offset = (content_h - viewport_h).max(0.0);
             let current_v_offset = self.breadcrumb_scroll.0.borrow().base_handle.offset().y.as_f32().abs();
             if current_v_offset > max_v_offset {
                  self.breadcrumb_scroll.0.borrow().base_handle.set_offset(Point::new(px(0.0), px(-max_v_offset)));
             }
        } else {
             self.breadcrumb_scroll.0.borrow().base_handle.set_offset(Point::new(px(0.0), px(0.0)));
        }

        // Clamp ContextMenu scroll
        if let Some((_, ContextMenuContent::Items(menu, _))) = &self.context_menu {
             let item_count = menu.len();
             let content_h = item_count as f32 * 28.0;
             let viewport_h = content_h.min(480.0);
             let max_v_offset = (content_h - viewport_h).max(0.0);
             let current_v_offset = self.context_menu_scroll.0.borrow().base_handle.offset().y.as_f32().abs();
             if current_v_offset > max_v_offset {
                  self.context_menu_scroll.0.borrow().base_handle.set_offset(Point::new(px(0.0), px(-max_v_offset)));
             }
        } else {
             self.context_menu_scroll.0.borrow().base_handle.set_offset(Point::new(px(0.0), px(0.0)));
        }
    }

    pub fn close_tab(&mut self, pane_id: &str, tab_idx: usize, cx: &mut Context<Self>) {
        self.state.close_tab_id(pane_id, tab_idx);
        cx.notify();
    }

    pub fn load_path_sync(&mut self, pane_id: &str, path: String, cx: &mut Context<Self>) {
        if let Some(pane) = self.state.find_pane_mut(pane_id) {
            let active_tab = pane.active_tab_mut();
            active_tab.path = path.clone();
            active_tab.files.clear();
            active_tab.filter_types.clear();
            active_tab.filter_name.clear();
            active_tab.selected_paths.clear();
            active_tab.list_scroll = UniformListScrollHandle::new(); 
            cx.notify();
            let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
            if let Ok(response) = rt.block_on(list_dir(path, None, None, None, None)) {
                if let Some(pane) = self.state.find_pane_mut(pane_id) {
                    let tab = pane.active_tab_mut();
                    tab.files = response.entries;
                    tab.sort_files();
                    tab.update_cache();
                    tab.list_scroll = UniformListScrollHandle::new();                }
            }
            cx.notify();
        }
    }

    pub fn update_filter_size(&mut self, pane_id: &str, category: crate::ui::workspace::SizeCategory, active: bool, cx: &mut Context<Self>) {
        if let Some(pane) = self.state.find_pane_mut(pane_id) {
            let tab = pane.active_tab_mut();
            if active {
                tab.filter_sizes.insert(category);
            } else {
                tab.filter_sizes.remove(&category);
            }
            tab.update_cache();
            cx.notify();
        }
    }

    pub fn open_type_filter_menu(&mut self, pane_id: &str, pos: Point<Pixels>, cx: &mut Context<Self>) {
        if let Some(pane) = self.state.find_pane(pane_id) {
            let tab = pane.active_tab();
            let tab_files = tab.files.clone();
            let current_filters = tab.filter_types.clone();
            let items = crate::ui::components::file_list::get_type_filter_items(self.handle.as_ref().unwrap().clone(), pane_id.to_string(), tab_files, current_filters, pos);
            self.context_menu = Some((pos, ContextMenuContent::Items(items, Some(ContextMenuHeader::TypeSelectAll { pane_id: pane_id.to_string() }))));
            cx.notify();
        }
    }

    pub fn open_size_filter_menu(&mut self, pane_id: &str, pos: Point<Pixels>, cx: &mut Context<Self>) {
        if let Some(pane) = self.state.find_pane(pane_id) {
            let tab = pane.active_tab();
            let tab_files = tab.files.clone();
            let current_filters = tab.filter_sizes.clone();
            let items = crate::ui::components::file_list::get_size_filter_items(self.handle.as_ref().unwrap().clone(), pane_id.to_string(), tab_files, current_filters, pos);
            self.context_menu = Some((pos, ContextMenuContent::Items(items, Some(ContextMenuHeader::SizeSelectAll { pane_id: pane_id.to_string() }))));
            cx.notify();
        }
    }

    pub fn update_filter_date(&mut self, pane_id: &str, category: crate::ui::workspace::DateCategory, active: bool, cx: &mut Context<Self>) {
        if let Some(pane) = self.state.find_pane_mut(pane_id) {
            let tab = pane.active_tab_mut();
            if active {
                tab.filter_dates.insert(category);
            } else {
                tab.filter_dates.remove(&category);
            }
            tab.update_cache();
            cx.notify();
        }
    }

    pub fn open_date_filter_menu(&mut self, pane_id: &str, pos: Point<Pixels>, cx: &mut Context<Self>) {
        if let Some(pane) = self.state.find_pane(pane_id) {
            let tab = pane.active_tab();
            let tab_files = tab.files.clone();
            let current_filters = tab.filter_dates.clone();
            let items = crate::ui::components::file_list::get_date_filter_items(self.handle.as_ref().unwrap().clone(), pane_id.to_string(), tab_files, current_filters, pos);
            self.context_menu = Some((pos, ContextMenuContent::Items(items, Some(ContextMenuHeader::DateSelectAll { pane_id: pane_id.to_string() }))));
            cx.notify();
        }
    }

    pub fn toggle_sort(&mut self, pane_id: &str, column: crate::ui::workspace::SortColumn, cx: &mut Context<Self>) {
        if let Some(pane) = self.state.find_pane_mut(pane_id) {
            let tab = pane.active_tab_mut();
            if tab.sort_column == column {
                tab.sort_direction = match tab.sort_direction {
                    crate::ui::workspace::SortDirection::Ascending => crate::ui::workspace::SortDirection::Descending,
                    crate::ui::workspace::SortDirection::Descending => crate::ui::workspace::SortDirection::Ascending,
                };
            } else {
                tab.sort_column = column;
                tab.sort_direction = crate::ui::workspace::SortDirection::Ascending;
            }
            tab.sort_files();
            tab.update_cache();
            cx.notify();
        }
    }

    pub fn toggle_filter_type(&mut self, pane_id: &str, filter: String, cx: &mut Context<Self>) {
        if let Some(pane) = self.state.find_pane_mut(pane_id) {
            let tab = pane.active_tab_mut();
            if tab.filter_types.contains(&filter) { tab.filter_types.remove(&filter); }
            else { tab.filter_types.insert(filter); }
            tab.update_cache();
            cx.notify();
        }
    }

    pub fn clear_filter_types(&mut self, pane_id: &str, cx: &mut Context<Self>) {
        if let Some(pane) = self.state.find_pane_mut(pane_id) {
            let tab = pane.active_tab_mut();
            tab.filter_types.clear();
            tab.update_cache();
            cx.notify();
        }
    }

    pub fn clear_filter_sizes(&mut self, pane_id: &str, cx: &mut Context<Self>) {
        if let Some(pane) = self.state.find_pane_mut(pane_id) {
            let tab = pane.active_tab_mut();
            tab.filter_sizes.clear();
            tab.update_cache();
            cx.notify();
        }
    }

    pub fn clear_filter_dates(&mut self, pane_id: &str, cx: &mut Context<Self>) {
        if let Some(pane) = self.state.find_pane_mut(pane_id) {
            let tab = pane.active_tab_mut();
            tab.filter_dates.clear();
            tab.update_cache();
            cx.notify();
        }
    }

    pub fn update_filter_name(&mut self, pane_id: &str, name: String, cx: &mut Context<Self>) {
        if let Some(pane) = self.state.find_pane_mut(pane_id) {
            let tab = pane.active_tab_mut();
            tab.filter_name = name;
            tab.update_cache();
            cx.notify();
        }
    }

    pub fn toggle_selection(&mut self, pane_id: &str, path: String, multi: bool, shift: bool, cx: &mut Context<Self>) {
        if let Some(pane) = self.state.find_pane_mut(pane_id) {
            let tab = pane.active_tab_mut();
            if shift {
                let filtered = tab.filtered_files();
                let anchor_idx = tab.selection_anchor.as_ref()
                    .and_then(|a| filtered.iter().position(|f| &f.path == a));
                let target_idx = filtered.iter().position(|f| f.path == path);

                if let (Some(start_idx), Some(end_idx)) = (anchor_idx, target_idx) {
                    let range_start = start_idx.min(end_idx);
                    let range_end = start_idx.max(end_idx);
                    
                    let paths_to_add: Vec<String> = filtered[range_start..=range_end].iter().map(|f| f.path.clone()).collect();

                    if !multi {
                        tab.selected_paths.clear();
                    }
                    for p in paths_to_add {
                        tab.selected_paths.insert(p);
                    }
                } else {
                    // No anchor or target not found, treat as single selection
                    if !multi { tab.selected_paths.clear(); }
                    tab.selected_paths.insert(path.clone());
                    tab.selection_anchor = Some(path);
                }
            } else if multi {
                if tab.selected_paths.contains(&path) {
                    tab.selected_paths.remove(&path);
                } else {
                    tab.selected_paths.insert(path.clone());
                    tab.selection_anchor = Some(path);
                }
            } else {
                tab.selected_paths.clear();
                tab.selected_paths.insert(path.clone());
                tab.selection_anchor = Some(path);
            }
            cx.notify();
        }
    }

    pub fn open_name_filter(&mut self, pane_id: &str, pos: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let h_cap = self.handle.as_ref().unwrap().clone();
        let pid_cap = pane_id.to_string();
        
        let initial_text = self.state.find_pane(pane_id)
            .map(|p: &crate::ui::workspace::PaneState| p.active_tab().filter_name.clone())
            .unwrap_or_default();
            
        let input = cx.new(|cx| {
            let h_change = h_cap.clone();
            let pid_change = pid_cap.clone();
            let h_enter = h_cap.clone();
            let h_esc = h_cap.clone();
            let pid_esc = pid_cap.clone();
            
            crate::ui::components::input::TextInput::new(cx, "Filtrer par nom...", initial_text,
                move |text, _, cx| { let _ = h_change.update(cx, |this, cx| this.update_filter_name(&pid_change, text.to_string(), cx)); },
                move |_, cx| { let _ = h_enter.update(cx, |this, _| { this.context_menu = None; }); },
                move |_, cx| {
                    let _ = h_esc.update(cx, |this, cx| {
                        this.update_filter_name(&pid_esc, "".to_string(), cx);
                        this.context_menu = None;
                    });
                }
            )
        });
        
        window.focus(&input.read(cx).focus_handle);
        
        self.context_menu = Some((pos, ContextMenuContent::NameFilter { input }));
        cx.notify();
    }

    fn render_layout_node(&self, node: &crate::ui::workspace::LayoutNode, handle: Entity<Self>, cx: &mut Context<Self>) -> AnyElement {
        match node {
            crate::ui::workspace::LayoutNode::Pane(pane) => {
                let p_id = pane.id.clone();
                div().flex_1().size_full().flex().relative()
                    .child(
                        canvas(
                            |bounds, _window, _cx| bounds,
                            {
                                let h = handle.clone();
                                let p_id = p_id.clone();
                                move |_, bounds: Bounds<Pixels>, _, cx| {
                                    let size = Size { width: bounds.size.width.as_f32(), height: bounds.size.height.as_f32() };
                                    let origin = Point { x: bounds.origin.x.as_f32(), y: bounds.origin.y.as_f32() };
                                    let _ = h.update(cx, |this, _| { 
                                        this.pane_sizes.insert(p_id.clone(), size); 
                                        this.pane_origins.insert(p_id.clone(), origin);
                                    });
                                }
                            }
                        ).absolute().size_full()
                    )
                    .child(self.render_pane(pane, handle, cx))
                    .into_any_element()
            }
            crate::ui::workspace::LayoutNode::Split { id, axis, children, weights } => {
                let total_weight: f32 = weights.iter().sum();
                let mut container = div().flex_1().size_full().flex()
                    .child(
                        canvas(
                            |bounds, _window, _cx| bounds,
                            {
                                let h = handle.clone();
                                let s_id = id.clone();
                                let axis_copy = axis.clone();
                                move |_, bounds: Bounds<Pixels>, _window, cx| {
                                    let size = match axis_copy {
                                        crate::ui::workspace::LayoutAxis::Horizontal => bounds.size.width.as_f32(),
                                        crate::ui::workspace::LayoutAxis::Vertical => bounds.size.height.as_f32(),
                                    };
                                    let _ = h.update(cx, |this, _| { this.split_sizes.insert(s_id.clone(), size); });
                                }
                            }
                        )
                        .absolute()
                        .size_full()
                    );

                match axis {
                    crate::ui::workspace::LayoutAxis::Horizontal => {
                        container = container.flex_row();
                    }
                    crate::ui::workspace::LayoutAxis::Vertical => {
                        container = container.flex_col();
                    }
                }
                
                for (i, child) in children.iter().enumerate() {
                    if i > 0 {
                        let separator = match axis {
                            crate::ui::workspace::LayoutAxis::Horizontal => {
                                div()
                                    .w(px(4.0)) // Wider grab area
                                    .h_full()
                                    .cursor(CursorStyle::ResizeLeftRight)
                                    .child(
                                        div()
                                            .size_full()
                                            .bg(cx.theme().border)
                                            .hover(|s| s.bg(cx.theme().accent))
                                    )
                                    .on_mouse_down(MouseButton::Left, {
                                        let h = handle.clone();
                                        let s_id = id.clone();
                                        let w_copy = weights.clone();
                                        let idx = i - 1;
                                        move |e, _, cx| {
                                            let _ = h.update(cx, |this, _| {
                                                this.drag_target = Some(DragTarget::PaneResize {
                                                    split_id: s_id.clone(),
                                                    child_idx: idx,
                                                    initial_weights: w_copy.clone(),
                                                });
                                                this.grab_offset = e.position.x.as_f32();
                                            });
                                        }
                                    })
                            }
                            crate::ui::workspace::LayoutAxis::Vertical => {
                                div()
                                    .h(px(4.0))
                                    .w_full()
                                    .cursor(CursorStyle::ResizeUpDown)
                                    .child(
                                        div()
                                            .size_full()
                                            .bg(cx.theme().border)
                                            .hover(|s| s.bg(cx.theme().accent))
                                    )
                                    .on_mouse_down(MouseButton::Left, {
                                        let h = handle.clone();
                                        let s_id = id.clone();
                                        let w_copy = weights.clone();
                                        let idx = i - 1;
                                        move |e, _, cx| {
                                            let _ = h.update(cx, |this, _| {
                                                this.drag_target = Some(DragTarget::PaneResize {
                                                    split_id: s_id.clone(),
                                                    child_idx: idx,
                                                    initial_weights: w_copy.clone(),
                                                });
                                                this.grab_offset = e.position.y.as_f32();
                                            });
                                        }
                                    })
                            }
                        };
                        container = container.child(separator);
                    }
                    container = container.child(
                        div()
                            .flex_basis(relative(weights[i] / total_weight.max(0.1)))
                            .flex_grow()
                            .flex_shrink()
                            .size_full()
                            .overflow_hidden()
                            .child(self.render_layout_node(child, handle.clone(), cx))
                    );
                }
                container.into_any_element()
            }
        }
    }

    fn render_pane(&self, pane: &crate::ui::workspace::PaneState, handle: Entity<Self>, cx: &mut Context<Self>) -> AnyElement {
        let h = handle.clone();
        let p_id = pane.id.clone();
        let _is_active = self.state.active_pane_id == p_id;
        
        div()
            .id(SharedString::from(format!("pane-container-{}", p_id.clone())))
            .relative()
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .child({
                let mut tab_bar = TabBar::new(SharedString::from(format!("tab-bar-{}", p_id)));
                let active_idx = pane.active_tab_index;
                let tabs_count = pane.tabs.len();

                for (t_ix, tab_data) in pane.tabs.iter().enumerate() {
                    let is_tab_active = t_ix == active_idx;
                    let h = h.clone();
                    let p_id_cap = p_id.clone();
                    
                    let label = if tab_data.path == "C:/" || tab_data.path == "C:\\" {
                        "C:".to_string()
                    } else {
                        tab_data.path.split(|c| c == '/' || c == '\\').filter(|p| !p.is_empty()).last().unwrap_or("C:").to_string()
                    };

                    let position = if t_ix == 0 {
                        TabPosition::First
                    } else if t_ix == tabs_count - 1 {
                        TabPosition::Last
                    } else {
                        TabPosition::Middle(t_ix.cmp(&active_idx))
                    };

                    let label_cap = label.clone();
                    let is_dragging = self.dragging_tab == Some((p_id.clone(), t_ix));
                    
                    let tab = Tab::new(SharedString::from(format!("tab-{}-{}", p_id, t_ix)), label.clone())
                        .selected(is_tab_active)
                        .position(position)
                        .icon("icons/folder.svg")
                        .on_click({
                            let h = h.clone();
                            let p_id_cap = p_id.clone();
                            move |_, cx| { let _ = h.update(cx, |this, cx| this.switch_tab(&p_id_cap, t_ix, cx)); }
                        })
                        .when(tabs_count > 1, |s| {
                            let h = h.clone();
                            let p_id_cap = p_id.clone();
                            s.on_close(move |_, cx| { let _ = h.update(cx, |this, cx| this.close_tab(&p_id_cap, t_ix, cx)); })
                        })
                        .on_drag((p_id.clone(), t_ix), {
                            let lbl = label_cap.clone();
                            let h = h.clone();
                            let p_id_cap = p_id.clone();
                            move |_window, cx| { 
                                 h.update(cx, |this, _| { this.dragging_tab = Some((p_id_cap.clone(), t_ix)); });
                                 cx.new(|_| TabDragPreview { label: lbl.clone() }) 
                            }
                        })
                        .is_dragging(is_dragging);

                    tab_bar = tab_bar.child(
                        div()
                            .relative()
                            .child(tab)
                            // Left Drop Zone
                            .child(
                                div()
                                    .absolute()
                                    .top_0()
                                    .left_0()
                                    .h_full()
                                    .w_1_2()
                                    .drag_over::<(String, usize)>({
                                        let h = h.clone();
                                        let p_id_cap = p_id.clone();
                                        move |style, (f_p, f_t), _, app| {
                                            let is_noop = f_p == &p_id_cap && (*f_t == t_ix || *f_t + 1 == t_ix);
                                            let _ = h.update(app, |v, _| {
                                                if is_noop {
                                                    v.drop_target_idx = None;
                                                    v.drop_target_pane = None;
                                                } else {
                                                    v.drop_target_idx = Some(t_ix);
                                                    v.drop_target_pane = Some(p_id_cap.clone());
                                                }
                                                // Clear split highlights when over a tab
                                                v.highlight_split_pane = None;
                                            });
                                            style
                                        }
                                    })
                                    .on_drop({
                                        let h = h.clone();
                                        let p_id_cap = p_id.clone();
                                        move |(from_p, from_t): &(String, usize), _, app| {
                                            let (f_p, f_t) = (from_p.clone(), *from_t);
                                            let _ = h.update(app, |this, cx| {
                                                if f_p == p_id_cap {
                                                    this.state.reorder_tabs_id(&p_id_cap, f_t, t_ix);
                                                } else {
                                                    this.state.move_tab_to_pane_id(&f_p, f_t, &p_id_cap);
                                                    if let Some(pane) = this.state.layout.find_pane(&p_id_cap) {
                                                        this.state.reorder_tabs_id(&p_id_cap, pane.tabs.len() - 1, t_ix);
                                                    }
                                                }
                                                this.drop_target_idx = None;
                                                this.drop_target_pane = None;
                                                this.dragging_tab = None;
                                                cx.notify();
                                            });
                                        }
                                    })
                            )
                            // Right Drop Zone
                            .child(
                                div()
                                    .absolute()
                                    .top_0()
                                    .right_0()
                                    .h_full()
                                    .w_1_2()
                                    .drag_over::<(String, usize)>({
                                        let h = h.clone();
                                        let p_id_cap = p_id.clone();
                                        move |style, (f_p, f_t), _, app| {
                                            // For Right Zone at index t_ix. Target is t_ix + 1.
                                            let target = t_ix + 1;
                                            let is_noop = f_p == &p_id_cap && (*f_t == target || *f_t + 1 == target);
                                            let _ = h.update(app, |v, _| {
                                                if is_noop {
                                                    v.drop_target_idx = None;
                                                    v.drop_target_pane = None;
                                                } else {
                                                    v.drop_target_idx = Some(target);
                                                    v.drop_target_pane = Some(p_id_cap.clone());
                                                }
                                                // Clear split highlights when over a tab
                                                v.highlight_split_pane = None;
                                            });
                                            style
                                        }
                                    })
                                    .on_drop({
                                        let h = h.clone();
                                        let p_id_cap = p_id.clone();
                                        move |(from_p, from_t): &(String, usize), _, app| {
                                            let (f_p, f_t) = (from_p.clone(), *from_t);
                                            let _ = h.update(app, |this, cx| {
                                                if f_p == p_id_cap {
                                                    this.state.reorder_tabs_id(&p_id_cap, f_t, t_ix + 1);
                                                } else {
                                                    this.state.move_tab_to_pane_id(&f_p, f_t, &p_id_cap);
                                                    if let Some(pane) = this.state.layout.find_pane(&p_id_cap) {
                                                        this.state.reorder_tabs_id(&p_id_cap, pane.tabs.len() - 1, t_ix + 1);
                                                    }
                                                }
                                                this.drop_target_idx = None;
                                                this.drop_target_pane = None;
                                                this.dragging_tab = None;
                                                cx.notify();
                                            });
                                        }
                                    })
                            )
                            // Visual Drop Indicator (Marker - Left side of this tab)
                            .child(
                                div()
                                    .absolute()
                                    .top_0()
                                    .left_0()
                                    .w(px(2.0))
                                    .h_full()
                                    .bg(rgb(0x0078d4))
                                    .when(self.drop_target_pane.as_ref() == Some(&p_id_cap) && self.drop_target_idx == Some(t_ix), |s| s.opacity(1.0))
                                    .when(!(self.drop_target_pane.as_ref() == Some(&p_id_cap) && self.drop_target_idx == Some(t_ix)), |s| s.opacity(0.0))
                            )
                            // Visual Drop Indicator (Marker - Right side of this tab, only for the last one)
                            .when(t_ix == tabs_count - 1, |s| s.child(
                                div()
                                    .absolute()
                                    .top_0()
                                    .right_0()
                                    .w(px(2.0))
                                    .h_full()
                                    .bg(rgb(0x0078d4))
                                    .when(self.drop_target_pane.as_ref() == Some(&p_id_cap) && self.drop_target_idx == Some(tabs_count), |s| s.opacity(1.0))
                                    .when(!(self.drop_target_pane.as_ref() == Some(&p_id_cap) && self.drop_target_idx == Some(tabs_count)), |s| s.opacity(0.0))
                            ))
                    );
                }

                // Gap at the end for dropping tabs
                tab_bar = tab_bar.child(
                    div()
                        .flex_1()
                        .min_w(px(20.0))
                        .h_full()
                        .relative()
                        .drag_over::<(String, usize)>({
                            let h = h.clone();
                            let p_id_cap = p_id.clone();
                            move |style, (from_p, from_t), _, app| {
                                let target = tabs_count;
                                // It's only a no-op if we are in the same pane AND we are already the last tab
                                let is_noop = from_p == &p_id_cap && *from_t == target.saturating_sub(1);
                                
                                let _ = h.update(app, |v, _| {
                                    if !is_noop {
                                        v.drop_target_idx = Some(target);
                                        v.drop_target_pane = Some(p_id_cap.clone());
                                    } else {
                                        v.drop_target_idx = None;
                                        v.drop_target_pane = None;
                                    }
                                    // Clear split highlights when over the gap
                                    v.highlight_split_pane = None;
                                });
                                style
                            }
                        })
                        .on_drop({
                            let h = h.clone();
                            let p_id_cap = p_id.clone();
                            move |(from_p, from_t): &(String, usize), _, app| {
                                let (f_p, f_t) = (from_p.clone(), *from_t);
                                let _ = h.update(app, |this, cx| {
                                    if f_p == p_id_cap {
                                        this.state.reorder_tabs_id(&p_id_cap, f_t, tabs_count);
                                    } else {
                                        this.state.move_tab_to_pane_id(&f_p, f_t, &p_id_cap);
                                    }
                                    this.drop_target_idx = None;
                                    this.drop_target_pane = None;
                                    this.dragging_tab = None;
                                    cx.notify();
                                });
                            }
                        })
                        .child(
                            div()
                                .absolute()
                                .top_0()
                                .left_0()
                                .w(px(2.0))
                                .h_full()
                                .bg(rgb(0x0078d4))
                                .when(self.drop_target_pane == Some(p_id.clone()) && self.drop_target_idx == Some(tabs_count), |s| s.opacity(1.0))
                                .when(!(self.drop_target_pane == Some(p_id.clone()) && self.drop_target_idx == Some(tabs_count)), |s| s.opacity(0.0))
                        )
                );
                div()
                    .h(px(32.0))
                    .w_full()
                    .relative()
                    .drag_over::<(String, usize)>({
                        let h = h.clone();
                        let p_id_cap = p_id.clone();
                        move |style, (f_p, f_t), _, app| {
                            let _ = h.update(app, |v, _| {
                                // If no specific tab or gap is already showing an indicator...
                                if v.drop_target_pane.as_ref() != Some(&p_id_cap) {
                                    if let Some(pane) = v.state.layout.find_pane(&p_id_cap) {
                                        let target = pane.tabs.len();
                                        let is_noop = f_p == &p_id_cap && *f_t == target.saturating_sub(1);
                                        if !is_noop {
                                            v.drop_target_idx = Some(target);
                                            v.drop_target_pane = Some(p_id_cap.clone());
                                        }
                                    }
                                }
                                // Clear split highlights when over the tab bar wrapper
                                v.highlight_split_pane = None;
                            });
                            style
                        }
                    })
                    .on_drop({
                        let h = h.clone();
                        let p_id_cap = p_id.clone();
                        move |(from_p, f_t): &(String, usize), _, app| {
                            let (f_p, f_t) = (from_p.clone(), *f_t);
                            let _ = h.update(app, |this, cx| {
                                // If we drop on the wrapper and it wasn't caught by a specific tab/gap
                                // then target the absolute end of the pane
                                if let Some(pane) = this.state.layout.find_pane(&p_id_cap) {
                                    let target = pane.tabs.len();
                                    if f_p == p_id_cap {
                                        this.state.reorder_tabs_id(&p_id_cap, f_t, target);
                                    } else {
                                        this.state.move_tab_to_pane_id(&f_p, f_t, &p_id_cap);
                                    }
                                }
                                this.drop_target_idx = None;
                                this.drop_target_pane = None;
                                this.dragging_tab = None;
                                cx.notify();
                            });
                        }
                    })
                    .child(tab_bar)
            })
            .child({
                div().id(SharedString::from(format!("pane-content-{}", p_id.clone()))).size_full().flex().flex_col().relative()
                    .child(render_top_bar(self, p_id.clone(), handle.clone(), cx))
                    .child(render_file_list(self, p_id.clone(), handle.clone(), cx))
                    // Visual highlights for splits
                    .when(self.highlight_split_pane == Some(p_id.clone()), |s: Stateful<Div>| {
                        s.child(div()
                            .absolute()
                            .size_full()
                            .when(self.highlight_left_zone, |s| s.child(div().absolute().left_0().h_full().w_1_2().bg(rgba(0x0078d422)).border_r_1().border_color(rgb(0x0078d4))))
                            .when(self.highlight_right_zone, |s| s.child(div().absolute().right_0().h_full().w_1_2().bg(rgba(0x0078d422)).border_l_1().border_color(rgb(0x0078d4))))
                            .when(self.highlight_top_zone, |s| s.child(div().absolute().top_0().w_full().h_1_2().bg(rgba(0x0078d422)).border_b_1().border_color(rgb(0x0078d4))))
                            .when(self.highlight_bottom_zone, |s| s.child(div().absolute().bottom_0().w_full().h_1_2().bg(rgba(0x0078d422)).border_t_1().border_color(rgb(0x0078d4))))
                        )
                    })
                    // Split detection zones
                    .child(div().absolute().left_0().h_full().w_1_4().drag_over::<(String, usize)>({
                        let h = h.clone();
                        let p_id_cap = p_id.clone();
                        move |style, _, _, app| {
                            let _ = h.update(app, |v, _| {
                                v.highlight_split_pane = Some(p_id_cap.clone());
                                v.highlight_left_zone = true;
                                v.highlight_right_zone = false; v.highlight_top_zone = false; v.highlight_bottom_zone = false;
                                
                                // Reset tab targeting when entering a split zone
                                v.drop_target_idx = None;
                                v.drop_target_pane = None;
                            });
                            style
                        }
                    }).on_drop({
                        let h = h.clone();
                        let p_id_cap = p_id.clone();
                        move |(from_p, f_t): &(String, usize), _, app| {
                            let (f_p, f_t) = (from_p.clone(), *f_t);
                            let _ = h.update(app, |this, cx| {
                                this.state.split_tab_to_axis(&f_p, f_t, &p_id_cap, crate::ui::workspace::LayoutAxis::Horizontal, false);
                                this.highlight_split_pane = None;
                                this.dragging_tab = None;
                                this.drop_target_idx = None;
                                this.drop_target_pane = None;
                                cx.notify();
                            });
                        }
                    }))
                    .child(div().absolute().right_0().h_full().w_1_4().drag_over::<(String, usize)>({
                        let h = h.clone();
                        let p_id_cap = p_id.clone();
                        move |style, _, _, app| {
                            let _ = h.update(app, |v, _| {
                                v.highlight_split_pane = Some(p_id_cap.clone());
                                v.highlight_right_zone = true;
                                v.highlight_left_zone = false; v.highlight_top_zone = false; v.highlight_bottom_zone = false;
                                
                                // Reset tab targeting when entering a split zone
                                v.drop_target_idx = None;
                                v.drop_target_pane = None;
                            });
                            style
                        }
                    }).on_drop({
                        let h = h.clone();
                        let p_id_cap = p_id.clone();
                        move |(from_p, f_t): &(String, usize), _, app| {
                            let (f_p, f_t) = (from_p.clone(), *f_t);
                            let _ = h.update(app, |this, cx| {
                                this.state.split_tab_to_axis(&f_p, f_t, &p_id_cap, crate::ui::workspace::LayoutAxis::Horizontal, true);
                                this.highlight_split_pane = None;
                                this.dragging_tab = None;
                                this.drop_target_idx = None;
                                this.drop_target_pane = None;
                                cx.notify();
                            });
                        }
                    }))
                    .child(div().absolute().top_0().w_full().h_1_4().drag_over::<(String, usize)>({
                        let h = h.clone();
                        let p_id_cap = p_id.clone();
                        move |style, _, _, app| {
                            let _ = h.update(app, |v, _| {
                                v.highlight_split_pane = Some(p_id_cap.clone());
                                v.highlight_top_zone = true;
                                v.highlight_left_zone = false; v.highlight_right_zone = false; v.highlight_bottom_zone = false;
                                
                                // Reset tab targeting when entering a split zone
                                v.drop_target_idx = None;
                                v.drop_target_pane = None;
                            });
                            style
                        }
                    }).on_drop({
                        let h = h.clone();
                        let p_id_cap = p_id.clone();
                        move |(from_p, f_t): &(String, usize), _, app| {
                            let (f_p, f_t) = (from_p.clone(), *f_t);
                            let _ = h.update(app, |this, cx| {
                                this.state.split_tab_to_axis(&f_p, f_t, &p_id_cap, crate::ui::workspace::LayoutAxis::Vertical, false);
                                this.highlight_split_pane = None;
                                this.dragging_tab = None;
                                this.drop_target_idx = None;
                                this.drop_target_pane = None;
                                cx.notify();
                            });
                        }
                    }))
                    .child(div().absolute().bottom_0().w_full().h_1_4().drag_over::<(String, usize)>({
                        let h = h.clone();
                        let p_id_cap = p_id.clone();
                        move |style, _, _, app| {
                            let _ = h.update(app, |v, _| {
                                v.highlight_split_pane = Some(p_id_cap.clone());
                                v.highlight_bottom_zone = true;
                                v.highlight_left_zone = false; v.highlight_right_zone = false; v.highlight_top_zone = false;
                                
                                // Reset tab targeting when entering a split zone
                                v.drop_target_idx = None;
                                v.drop_target_pane = None;
                            });
                            style
                        }
                    }).on_drop({
                        let h = h.clone();
                        let p_id_cap = p_id.clone();
                        move |(from_p, f_t): &(String, usize), _, app| {
                            let (f_p, f_t) = (from_p.clone(), *f_t);
                            let _ = h.update(app, |this, cx| {
                                this.state.split_tab_to_axis(&f_p, f_t, &p_id_cap, crate::ui::workspace::LayoutAxis::Vertical, true);
                                this.highlight_split_pane = None;
                                this.dragging_tab = None;
                                this.drop_target_idx = None;
                                this.drop_target_pane = None;
                                cx.notify();
                            });
                        }
                    }))
            })
            .into_any_element()
    }
}

impl Render for OxydeApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let handle = self.handle.clone().expect("handle set");

        let size = _window.viewport_size();
        let win_w = size.width.as_f32();
        let win_h = size.height.as_f32();

        self.list_width = (win_w - self.sidebar_width - 4.0).max(1.0);
        self.list_height = win_h.max(1.0);

        if let Some(DragTarget::SelectionRectangle { ref pane_id, current_pos, .. }) = self.drag_target {
            let p_id = pane_id.clone();
            let pos = current_pos;
            self.handle_selection_drag(p_id, pos, cx);
        }

        self.clamp_scrolls();

        let mut root = div()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().text)
            .font_family("Inter")
            .flex()
            .text_size(px(13.0))
            .font_weight(FontWeight::LIGHT);
        
        root.text_style().get_or_insert_with(Default::default).font_features = Some(TNUM.clone());
        
        let _handle_wheel = handle.clone();
        
        let global_handle = handle.clone();
        root = root.child(
            canvas(
                |_bounds, _window, _cx| (),
                move |_bounds, _prepaint, _window, _cx| {
                    _window.on_mouse_event({
                        let handle_move = global_handle.clone();
                        move |e: &MouseMoveEvent, phase, _, cx| {
                            if phase != DispatchPhase::Capture && phase != DispatchPhase::Bubble { return; }
                            let _ = handle_move.update(cx, |this: &mut OxydeApp, cx: &mut Context<OxydeApp>| {
                                if let Some(target) = this.drag_target.clone() {
                                    let pos = e.position;
                                    match target {
                                        DragTarget::SidebarV { viewport_h, content_h, track_id } => {
                                              if let Some(origin) = this.scrollbar_origins.get(&track_id) {
                                                  let mouse_pos_in_track = pos.y.as_f32() - origin.y;
                                                  let thumb_size = (viewport_h * (viewport_h / content_h.max(1.0)).min(1.0)).max(40.0);
                                                  let travel_distance = (viewport_h - thumb_size).max(1.0);
                                                  let content_range = content_h - viewport_h;
                                                  let target_thumb_pos = (mouse_pos_in_track - this.grab_offset).clamp(0.0, travel_distance);
                                                  let target_px = (target_thumb_pos / travel_distance) * content_range;
                                                  this.sidebar_scroll.0.borrow().base_handle.set_offset(Point::new(px(0.0), px(-target_px)));
                                                  cx.notify();
                                              }
                                        }
                                        DragTarget::SidebarH { viewport_w, content_w, track_id } => {
                                              if let Some(origin) = this.scrollbar_origins.get(&track_id) {
                                                  let mouse_pos_in_track = pos.x.as_f32() - origin.x;
                                                  let thumb_size = (viewport_w * (viewport_w / content_w.max(1.0)).min(1.0)).max(40.0);
                                                  let travel_distance = (viewport_w - thumb_size).max(1.0);
                                                  let content_range = content_w - viewport_w;
                                                  let target_thumb_pos = (mouse_pos_in_track - this.grab_offset).clamp(0.0, travel_distance);
                                                  let target_px = (target_thumb_pos / travel_distance) * content_range;
                                                  this.sidebar_scroll_h.set_offset(Point::new(px(-target_px), px(0.0)));
                                                  cx.notify();
                                              }
                                        }
                                        DragTarget::SidebarResize => {
                                              this.sidebar_width = (pos.x.as_f32() - this.grab_offset).clamp(150.0, 600.0);
                                              cx.notify();
                                        }
                                        DragTarget::FileListV { pane_id, viewport_h, content_h, track_id } => {
                                              if let Some(origin) = this.scrollbar_origins.get(&track_id) {
                                                  if let Some(pane) = this.state.layout.find_pane_mut(&pane_id) {
                                                      let mouse_pos_in_track = pos.y.as_f32() - origin.y;
                                                      let thumb_size = (viewport_h * (viewport_h / content_h.max(1.0)).min(1.0)).max(40.0);
                                                      let travel_distance = (viewport_h - thumb_size).max(1.0);
                                                      let content_range = content_h - viewport_h;
                                                      let target_thumb_pos = (mouse_pos_in_track - this.grab_offset).clamp(0.0, travel_distance);
                                                      let target_px = (target_thumb_pos / travel_distance) * content_range;
                                                      pane.active_tab_mut().list_scroll.0.borrow().base_handle.set_offset(Point::new(px(0.0), px(-target_px)));
                                                      cx.notify();
                                                  }
                                              }
                                        }
                                        DragTarget::FileListH { pane_id, viewport_w, content_w, track_id } => {
                                                 if let Some(origin) = this.scrollbar_origins.get(&track_id) {
                                                     if let Some(pane) = this.state.layout.find_pane_mut(&pane_id) {
                                                         let mouse_pos_in_track = pos.x.as_f32() - origin.x;
                                                         let thumb_size = (viewport_w * (viewport_w / content_w.max(1.0)).min(1.0)).max(40.0);
                                                         let travel_distance = (viewport_w - thumb_size).max(1.0);
                                                         let content_range = content_w - viewport_w;
                                                         let target_thumb_pos = (mouse_pos_in_track - this.grab_offset).clamp(0.0, travel_distance);
                                                         let target_px = (target_thumb_pos / travel_distance) * content_range;
                                                         pane.active_tab_mut().list_scroll_h.set_offset(Point::new(px(-target_px), px(0.0)));
                                                         cx.notify();
                                                     }
                                                 }
                                        }
                                        DragTarget::BreadcrumbV { viewport_h, content_h, track_id } => {
                                              if let Some(origin) = this.scrollbar_origins.get(&track_id) {
                                                  let mouse_pos_in_track = pos.y.as_f32() - origin.y;
                                                  let thumb_size = (viewport_h * (viewport_h / content_h.max(1.0)).min(1.0)).max(40.0);
                                                  let travel_distance = (viewport_h - thumb_size).max(1.0);
                                                  let content_range = content_h - viewport_h;
                                                  let target_thumb_pos = (mouse_pos_in_track - this.grab_offset).clamp(0.0, travel_distance);
                                                  let target_px = (target_thumb_pos / travel_distance) * content_range;
                                                  this.breadcrumb_scroll.0.borrow().base_handle.set_offset(Point::new(px(0.0), px(-target_px)));
                                                  cx.notify();
                                              }
                                        }
                                         DragTarget::ContextMenuV { viewport_h, content_h, track_id } => {
                                               if let Some(origin) = this.scrollbar_origins.get(&track_id) {
                                                   let mouse_pos_in_track = pos.y.as_f32() - origin.y;
                                                   let thumb_size = (viewport_h * (viewport_h / content_h.max(1.0)).min(1.0)).max(40.0);
                                                   let travel_distance = (viewport_h - thumb_size).max(1.0);
                                                   let content_range = content_h - viewport_h;
                                                   let target_thumb_pos = (mouse_pos_in_track - this.grab_offset).clamp(0.0, travel_distance);
                                                   let target_px = (target_thumb_pos / travel_distance) * content_range;
                                                   this.context_menu_scroll.0.borrow().base_handle.set_offset(Point::new(px(0.0), px(-target_px)));
                                                   cx.notify();
                                               }
                                         }
                                        DragTarget::PaneResize { split_id, child_idx, initial_weights } => {
                                             if let Some(total_size) = this.split_sizes.get(&split_id).cloned() {
                                                  if let Some((grid_handle, weights)) = this.state.layout.find_split_mut(&split_id) {
                                                      let current_pos = match grid_handle {
                                                          crate::ui::workspace::LayoutAxis::Horizontal => pos.x.as_f32(),
                                                          crate::ui::workspace::LayoutAxis::Vertical => pos.y.as_f32(),
                                                      };
                                                      let delta = current_pos - this.grab_offset;
                                                      let total_weight: f32 = initial_weights.iter().sum();
                                                      let weight_per_pixel = total_weight / total_size.max(1.0);
                                                      let delta_weight = delta * weight_per_pixel;
                                                      
                                                      let i = child_idx;
                                                      let j = i + 1;
                                                      
                                                      if i < weights.len() && j < weights.len() {
                                                          let total_pair_weight = initial_weights[i] + initial_weights[j];
                                                          let new_wi = (initial_weights[i] + delta_weight).clamp(0.1, total_pair_weight - 0.1);
                                                          let new_wj = total_pair_weight - new_wi;
                                                          
                                                          weights[i] = new_wi;
                                                          weights[j] = new_wj;
                                                          cx.notify();
                                                      }
                                                  }
                                             }
                                        }
                                        DragTarget::SelectionRectangle { ref pane_id, start_pos, modifiers, ref initial_selection, .. } => {
                                             let current_pos = pos;
                                             let initial_selection_cap = initial_selection.clone();
                                             this.drag_target = Some(DragTarget::SelectionRectangle {
                                                 pane_id: pane_id.clone(),
                                                 start_pos,
                                                 current_pos,
                                                 modifiers,
                                                 initial_selection: initial_selection_cap,
                                             });
                                             cx.notify();
                                        }
                                    }
                                }
                            });
                        }
                    });

                    _window.on_mouse_event({
                        let handle_up = global_handle.clone();
                        move |_: &MouseUpEvent, phase, _, cx| {
                            if phase != DispatchPhase::Capture && phase != DispatchPhase::Bubble { return; }
                            let _ = handle_up.update(cx, |this: &mut OxydeApp, cx: &mut Context<OxydeApp>| {
                                let mut needs_notify = false;
                                if this.drag_target.is_some() {
                                    this.drag_target = None;
                                    needs_notify = true;
                                }
                                if this.dragging_tab.is_some() {
                                    this.dragging_tab = None;
                                    needs_notify = true;
                                }
                                if this.drop_target_idx.is_some() || this.drop_target_pane.is_some() {
                                    this.drop_target_idx = None;
                                    this.drop_target_pane = None;
                                    needs_notify = true;
                                }
                                if this.highlight_split_pane.is_some() {
                                    this.highlight_split_pane = None;
                                    needs_notify = true;
                                }
                                if needs_notify {
                                    cx.notify();
                                }
                            });
                        }
                    });
                }
            ).size_full().absolute()
        );


        root = root
            .on_mouse_down(MouseButton::Navigate(NavigationDirection::Back), {
                let h = handle.clone();
                move |_, _, cx| { let _ = h.update(cx, |v, cx| {
                    let p_id = v.state.active_pane_id.clone();
                    v.go_back(&p_id, cx)
                }); }
            })
            .on_mouse_down(MouseButton::Navigate(NavigationDirection::Forward), {
                let h = handle.clone();
                move |_, _, cx| { let _ = h.update(cx, |v, cx| {
                    let p_id = v.state.active_pane_id.clone();
                    v.go_forward(&p_id, cx)
                }); }
            })
            .child(
                div()
                    .w(px(self.sidebar_width))
                    .h_full()
                    .flex_none()
                    .child(render_sidebar(self, handle.clone(), cx))
            )
            .child(
                div()
                    .id("sidebar-resize-handle")
                    .w(px(4.0))
                    .h_full()
                    .bg(rgb(0x1a1a1a))
                    .cursor_col_resize()
                    .hover(|s| s.bg(rgb(0x0078d4)))
                    .active(|s| s.bg(rgb(0x0078d4)))
                    .on_mouse_down(MouseButton::Left, {
                        let h = handle.clone();
                        move |e, _, cx| {
                            let _ = h.update(cx, |this, _| {
                                this.drag_target = Some(DragTarget::SidebarResize);
                                this.grab_offset = e.position.x.as_f32() - this.sidebar_width;
                            });
                        }
                    })
            );
        
        let layout_tree = self.state.layout.clone();
        root = root.child(div().flex_1().h_full().child(self.render_layout_node(&layout_tree, handle.clone(), cx)));
        
        // CONTEXT MENU OVERLAY
        root = root.child(
            div()
                .when_some(self.context_menu.clone(), |this, (pos, content)| {
                    let h = handle.clone();
                    let pos = pos;
                    
                    let (items, header) = match content {
                        ContextMenuContent::Items(items, header_type) => {
                            let header = header_type.as_ref().map(|ht| {
                                match ht {
                                    ContextMenuHeader::TypeSelectAll { pane_id } => {
                                        let pid_cap = pane_id.clone();
                                        let h_inner = h.clone();
                                        
                                        // Dynamic label and action based on current state
                                        let (label, should_select) = if let Some(pane) = self.state.find_pane(&pid_cap) {
                                            let tab = pane.active_tab();
                                            let mut all_present = std::collections::HashSet::new();
                                            for f in &tab.files {
                                                if f.is_dir { all_present.insert("Dossier".to_string()); }
                                                else if let Some(ext) = std::path::Path::new(&f.name).extension().and_then(|e| e.to_str()) {
                                                    all_present.insert(ext.to_uppercase());
                                                }
                                            }
                                            let is_full = !all_present.is_empty() && all_present.iter().all(|t| tab.filter_types.contains(t));
                                            if is_full { ("Tout décocher", false) } else { ("Tout cocher", true) }
                                        } else { ("Tout cocher", true) };

                                        crate::ui::elements::button::Button::new(SharedString::from(format!("select-all-type-{}", pid_cap)))
                                            .label(label)
                                            .variant(crate::ui::elements::button::ButtonVariant::Subtle)
                                            .on_click(move |_, cx| {
                                                let pid_inner = pid_cap.clone();
                                                let _ = h_inner.update(cx, |this, cx| {
                                                    if let Some(pane) = this.state.find_pane_mut(&pid_inner) {
                                                        let tab = pane.active_tab_mut();
                                                        if should_select {
                                                            let all_files = tab.files.clone();
                                                            for f in &all_files {
                                                                if f.is_dir { tab.filter_types.insert("Dossier".to_string()); }
                                                                else if let Some(ext) = std::path::Path::new(&f.name).extension().and_then(|e| e.to_str()) {
                                                                    tab.filter_types.insert(ext.to_uppercase());
                                                                }
                                                            }
                                                        } else {
                                                            this.clear_filter_types(&pid_inner, cx);
                                                        }
                                                    }
                                                    this.open_type_filter_menu(&pid_inner, pos, cx);
                                                });
                                            })
                                            .into_any_element()
                                    }
                                    ContextMenuHeader::SizeSelectAll { pane_id } => {
                                        let pid_cap = pane_id.clone();
                                        let h_inner = h.clone();
                                        
                                        let (label, should_select) = if let Some(p) = self.state.find_pane(&pid_cap) {
                                            use crate::ui::workspace::SizeCategory;
                                            let tab = p.active_tab();
                                            let categories = [SizeCategory::Minuscule, SizeCategory::Petit, SizeCategory::Moyen, SizeCategory::Grand, SizeCategory::Huge];
                                            let mut present = std::collections::HashSet::new();
                                            for f in &tab.files {
                                                for cat in &categories {
                                                    if cat.matches(f.size) { present.insert(*cat); break; }
                                                }
                                            }
                                            let is_full = !present.is_empty() && present.iter().all(|c| tab.filter_sizes.contains(c));
                                            if is_full { ("Tout décocher", false) } else { ("Tout cocher", true) }
                                        } else { ("Tout cocher", true) };

                                        crate::ui::elements::button::Button::new(SharedString::from(format!("select-all-size-{}", pid_cap)))
                                            .label(label)
                                            .variant(crate::ui::elements::button::ButtonVariant::Subtle)
                                            .on_click(move |_, cx| {
                                                let pid_inner = pid_cap.clone();
                                                let _ = h_inner.update(cx, |this, cx| {
                                                    if let Some(p) = this.state.find_pane_mut(&pid_inner) {
                                                        use crate::ui::workspace::SizeCategory;
                                                        let tab = p.active_tab_mut();
                                                        if should_select {
                                                            tab.filter_sizes.insert(SizeCategory::Minuscule);
                                                            tab.filter_sizes.insert(SizeCategory::Petit);
                                                            tab.filter_sizes.insert(SizeCategory::Moyen);
                                                            tab.filter_sizes.insert(SizeCategory::Grand);
                                                            tab.filter_sizes.insert(SizeCategory::Huge);
                                                        } else {
                                                            this.clear_filter_sizes(&pid_inner, cx);
                                                        }
                                                    }
                                                    this.open_size_filter_menu(&pid_inner, pos, cx);
                                                });
                                            })
                                            .into_any_element()
                                    }
                                    ContextMenuHeader::DateSelectAll { pane_id } => {
                                        let pid_cap = pane_id.clone();
                                        let h_inner = h.clone();
                                        
                                        let (label, should_select) = if let Some(p) = self.state.find_pane(&pid_cap) {
                                            use crate::ui::workspace::DateCategory;
                                            let tab = p.active_tab();
                                            let categories = [DateCategory::Aujourdhui, DateCategory::Hier, DateCategory::CetteSemaine, DateCategory::LaSemaineDerniere, DateCategory::PlusAncien];
                                            let mut present = std::collections::HashSet::new();
                                            for f in &tab.files {
                                                for cat in &categories {
                                                    if cat.matches(f.modified) { present.insert(*cat); break; }
                                                }
                                            }
                                            let is_full = !present.is_empty() && present.iter().all(|c| tab.filter_dates.contains(c));
                                            if is_full { ("Tout décocher", false) } else { ("Tout cocher", true) }
                                        } else { ("Tout cocher", true) };

                                        crate::ui::elements::button::Button::new(SharedString::from(format!("select-all-date-{}", pid_cap)))
                                            .label(label)
                                            .variant(crate::ui::elements::button::ButtonVariant::Subtle)
                                            .on_click(move |_, cx| {
                                                let pid_inner = pid_cap.clone();
                                                let _ = h_inner.update(cx, |this, cx| {
                                                    if let Some(p) = this.state.find_pane_mut(&pid_inner) {
                                                        use crate::ui::workspace::DateCategory;
                                                        let tab = p.active_tab_mut();
                                                        if should_select {
                                                            tab.filter_dates.insert(DateCategory::Aujourdhui);
                                                            tab.filter_dates.insert(DateCategory::Hier);
                                                            tab.filter_dates.insert(DateCategory::CetteSemaine);
                                                            tab.filter_dates.insert(DateCategory::LaSemaineDerniere);
                                                            tab.filter_dates.insert(DateCategory::PlusAncien);
                                                        } else {
                                                            this.clear_filter_dates(&pid_inner, cx);
                                                        }
                                                    }
                                                    this.open_date_filter_menu(&pid_inner, pos, cx);
                                                });
                                            })
                                            .into_any_element()
                                    }
                                }
                            });
                            (items.iter().map(|it| {
                                let on_click = it.on_click.clone();
                                crate::ui::components::dropdown::DropdownItem {
                                    label: it.label.clone(),
                                    icon: it.icon.clone(),
                                    stay_open: it.stay_open,
                                    on_click: std::rc::Rc::new(move |window, app| { (on_click)(window, app); })
                                }
                            }).collect(), header)
                        },
                        ContextMenuContent::NameFilter { input } => {
                            (vec![], Some(input.clone().into_any_element()))
                        }
                    };

                    let scroll_handle = self.context_menu_scroll.clone();
                    let is_v_dragging = matches!(self.drag_target, Some(DragTarget::ContextMenuV { .. }));
                    
                    this.child(
                        deferred(
                            anchored()
                                .position(pos)
                                .child(
                                    cx.new(|_| crate::ui::components::dropdown::GenericDropdown {
                                        h_app: h.clone(),
                                        items,
                                        scroll_handle,
                                        width: None,
                                        max_height: 480.0,
                                        on_close: {
                                            let h = h.clone();
                                            std::rc::Rc::new(move |_, cx| {
                                                let _ = h.update(cx, |this, _| { this.context_menu = None; });
                                            })
                                        },
                                        on_drag: {
                                            std::rc::Rc::new(move |this, tid, vh, ch, _| {
                                                this.drag_target = Some(DragTarget::ContextMenuV { viewport_h: vh, content_h: ch, track_id: tid });
                                            })
                                        },
                                        is_dragging: is_v_dragging,
                                        header,
                                    })
                                )
                        )
                    )
                })
        );

        root
    }
}
