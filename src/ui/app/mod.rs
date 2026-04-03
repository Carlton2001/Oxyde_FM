use gpui::*;
pub use gpui::prelude::*;
pub use crate::ui::theme::{PixelsExt, ThemeContext};
pub use crate::ui::workspace::WorkspaceState;
pub use oxyde_lib::models::FileEntry;

pub mod navigation;
pub mod selection;
pub mod filters;
pub mod layout;

pub use navigation::*;
pub use selection::*;
pub use filters::*;
pub use layout::*;

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
}

impl Render for OxydeApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use crate::ui::components::sidebar::render_sidebar;
        use crate::ui::style::TNUM;

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
                    use crate::ui::components::dropdown::{GenericDropdown};

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
                                    cx.new(|_| GenericDropdown {
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
