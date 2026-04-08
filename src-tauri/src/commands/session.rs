use crate::models::{CommandError, SessionManager, SessionState, Tab};
use crate::models::session::{PanelState, SortConfig, LayoutNode, LayoutAxis};
use tauri::{AppHandle, Emitter, State};
use std::path::PathBuf;
use std::sync::MutexGuard;
use uuid::Uuid;

/// Helper to lock session state.
fn lock_session(state: &SessionManager) -> Result<MutexGuard<'_, SessionState>, CommandError> {
    state.0.lock().map_err(|_| CommandError::SystemError("Failed to lock session state".into()))
}

#[tauri::command]
pub fn get_session_state(state: State<'_, SessionManager>) -> Result<SessionState, CommandError> {
    let session = lock_session(&state)?;
    Ok(session.clone())
}

#[tauri::command]
pub fn create_tab(
    app: AppHandle,
    state: State<'_, SessionManager>,
    panel_id: String,
    path: String,
    background: Option<bool>,
) -> Result<String, CommandError> {
    let mut session = lock_session(&state)?;
    
    let new_id = Uuid::new_v4().to_string();
    let new_tab = Tab {
        id: new_id.clone(),
        path: PathBuf::from(&path),
        version: 0,
    };

    let panel = session.try_get_panel_mut(&panel_id)?;
    panel.tabs.push(new_tab);
    
    let is_background = background.unwrap_or(false);
    if !is_background {
        panel.active_tab_id = new_id.clone();
    }
    
    panel.update_watcher(&app);

    if !is_background {
        session.active_panel_id = panel_id;
    }

    app.emit("session_changed", session.clone()).map_err(|e| CommandError::SystemError(e.to_string()))?;
    drop(session);
    state.save(&app)?;
    
    Ok(new_id)
}

#[tauri::command]
pub fn close_other_tabs(
    app: AppHandle,
    state: State<'_, SessionManager>,
    tab_id: String,
) -> Result<(), CommandError> {
    let mut session = lock_session(&state)?;
    
    for panel in session.root.all_panels_mut() {
        if panel.tabs.iter().any(|t| t.id == tab_id) {
            panel.tabs.retain(|t| t.id == tab_id);
            panel.active_tab_id = tab_id.clone();
            panel.update_watcher(&app);
            break;
        }
    }

    app.emit("session_changed", session.clone()).map_err(|e| CommandError::SystemError(e.to_string()))?;
    drop(session);
    state.save(&app)?;
    Ok(())
}

#[tauri::command]
pub fn clear_all_panels_except(
    app: AppHandle,
    state: State<'_, SessionManager>,
    panel_id: String,
) -> Result<(), CommandError> {
    let mut session = lock_session(&state)?;
    
    if let Some(panel_state) = session.root.find_pane_mut(&panel_id) {
        let state_copy = panel_state.clone();
        session.root = LayoutNode::Pane { id: panel_id.clone(), state: state_copy };
        session.active_panel_id = panel_id;
    }

    app.emit("session_changed", session.clone()).map_err(|e| CommandError::SystemError(e.to_string()))?;
    drop(session);
    state.save(&app)?;
    Ok(())
}

#[tauri::command]
pub fn close_tab(
    app: AppHandle,
    state: State<'_, SessionManager>,
    tab_id: String,
) -> Result<(), CommandError> {
    let mut session = lock_session(&state)?;
    
    for panel in session.root.all_panels_mut() {
        if let Some(pos) = panel.tabs.iter().position(|t| t.id == tab_id) {
            panel.tabs.remove(pos);
            if panel.active_tab_id == tab_id {
                let new_pos = pos.min(panel.tabs.len().saturating_sub(1));
                if let Some(next_tab) = panel.tabs.get(new_pos) {
                    panel.active_tab_id = next_tab.id.clone();
                } else {
                    let default_id = Uuid::new_v4().to_string();
                    panel.tabs.push(Tab {
                        id: default_id.clone(),
                        path: PathBuf::from("C:\\"),
                        version: 0,
                    });
                    panel.active_tab_id = default_id;
                }
            }
            panel.update_watcher(&app);
        }
    }

    app.emit("session_changed", session.clone()).map_err(|e| CommandError::SystemError(e.to_string()))?;
    drop(session);
    state.save(&app)?;
    Ok(())
}

#[tauri::command]
pub fn switch_tab(
    app: AppHandle,
    state: State<'_, SessionManager>,
    tab_id: String,
) -> Result<(), CommandError> {
    let mut session = lock_session(&state)?;

    let mut found_panel_id = None;
    
    // We need to find which pane contains this tab. 
    // LayoutNode doesn't easily give us the ID of the pane containing a tab without traversal.
    fn find_pane_id_with_tab(node: &LayoutNode, tab_id: &str) -> Option<String> {
        match node {
            LayoutNode::Pane { id, state } => {
                if state.tabs.iter().any(|t| t.id == tab_id) {
                    Some(id.clone())
                } else {
                    None
                }
            }
            LayoutNode::Split { children, .. } => {
                for child in children {
                    if let Some(id) = find_pane_id_with_tab(child, tab_id) {
                        return Some(id);
                    }
                }
                None
            }
        }
    }

    if let Some(panel_id) = find_pane_id_with_tab(&session.root, &tab_id) {
        let panel = session.try_get_panel_mut(&panel_id)?;
        panel.active_tab_id = tab_id;
        panel.update_watcher(&app);
        session.active_panel_id = panel_id;
        found_panel_id = Some(());
    }

    if found_panel_id.is_none() {
        return Err(CommandError::Other("Tab not found".to_string()));
    }

    app.emit("session_changed", session.clone()).map_err(|e| CommandError::SystemError(e.to_string()))?;
    drop(session);
    state.save(&app)?;
    Ok(())
}

#[tauri::command]
pub fn active_tab_navigate(
    app: AppHandle,
    state: State<'_, SessionManager>,
    panel_id: String,
    path: String,
    version: Option<u64>,
) -> Result<(), CommandError> {
    let mut session = lock_session(&state)?;
    
    {
        let panel = session.try_get_panel_mut(&panel_id)?;
        if let Some(tab) = panel.tabs.iter_mut().find(|t| t.id == panel.active_tab_id) {
            if let Some(v) = version {
                if v > tab.version {
                    tab.path = PathBuf::from(&path);
                    tab.version = v;
                } else if v < tab.version {
                    return Ok(());
                } else if tab.path.to_string_lossy() != path {
                    tab.path = PathBuf::from(&path);
                }
            } else {
                tab.path = PathBuf::from(path);
                tab.version += 1;
            }
        }
        panel.update_watcher(&app);
    }

    app.emit("session_changed", session.clone()).map_err(|e| CommandError::SystemError(e.to_string()))?;
    drop(session);
    state.save(&app)?;
    Ok(())
}

#[tauri::command]
pub fn set_active_panel(
    app: AppHandle,
    state: State<'_, SessionManager>,
    panel_id: String,
) -> Result<(), CommandError> {
    let mut session = lock_session(&state)?;
    
    // Validate existence
    if session.root.find_pane_mut(&panel_id).is_none() {
         return Err(CommandError::Other("Invalid panel ID".to_string()));
    }

    session.active_panel_id = panel_id;
    app.emit("session_changed", session.clone()).map_err(|e| CommandError::SystemError(e.to_string()))?;
    drop(session);
    state.save(&app)?;
    Ok(())
}

#[tauri::command]
pub fn duplicate_tab(
    app: AppHandle,
    state: State<'_, SessionManager>,
    tab_id: String,
) -> Result<(), CommandError> {
    let mut session = lock_session(&state)?;

    for panel in session.root.all_panels_mut() {
        if let Some(pos) = panel.tabs.iter().position(|t| t.id == tab_id) {
            let tab = &panel.tabs[pos];
            let new_tab = Tab {
                id: Uuid::new_v4().to_string(),
                path: tab.path.clone(),
                version: tab.version,
            };
            panel.tabs.insert(pos + 1, new_tab.clone());
            panel.active_tab_id = new_tab.id;
            panel.update_watcher(&app);
        }
    }

    app.emit("session_changed", session.clone()).map_err(|e| CommandError::SystemError(e.to_string()))?;
    drop(session);
    state.save(&app)?;
    Ok(())
}

#[tauri::command]
pub fn split_panel(
    app: AppHandle,
    state: State<'_, SessionManager>,
    source_panel_id: String,
    target_panel_id: String,
    tab_id: String,
    side: String, // "top", "bottom", "left", "right"
) -> Result<(), CommandError> {
    let mut session = lock_session(&state)?;
    
    // 1. Extract the tab from source
    let mut moved_tab = None;
    let mut source_became_empty = false;

    for panel in session.root.all_panels_mut() {
        if let Some(pos) = panel.tabs.iter().position(|t| t.id == tab_id) {
            moved_tab = Some(panel.tabs.remove(pos));
            if panel.tabs.is_empty() {
                source_became_empty = true;
            } else if panel.active_tab_id == tab_id {
                let new_pos = pos.min(panel.tabs.len().saturating_sub(1));
                panel.active_tab_id = panel.tabs[new_pos].id.clone();
            }
            panel.update_watcher(&app);
            break;
        }
    }

    let tab = moved_tab.ok_or_else(|| CommandError::Other(format!("Tab {} not found", tab_id)))?;
    
    // Find the source panel's sort_config before we moved the tab
    let source_sort_config = session.root.find_pane(&source_panel_id)
        .map(|p| p.sort_config.clone())
        .unwrap_or_default();

    // 2. Create new pane
    let new_panel_id = format!("panel-{}", Uuid::new_v4().to_string().get(..8).unwrap_or("extra"));
    let new_pane = LayoutNode::Pane {
        id: new_panel_id.clone(),
        state: PanelState {
            active_tab_id: tab.id.clone(),
            tabs: vec![tab],
            watcher: None,
            watched_path: None,
            search_context: None,
            sort_config: source_sort_config,
            cached_results: None,
        },
    };

    // 3. Determine axis and order
    let (axis, after) = match side.as_str() {
        "top" => (LayoutAxis::Vertical, false),
        "bottom" => (LayoutAxis::Vertical, true),
        "left" => (LayoutAxis::Horizontal, false),
        "right" => (LayoutAxis::Horizontal, true),
        _ => (LayoutAxis::Horizontal, true),
    };

    // 4. Perform recursive split
    fn perform_split(node: &mut LayoutNode, target_id: &str, new_node: LayoutNode, axis: LayoutAxis, after: bool) -> bool {
        match node {
            LayoutNode::Pane { id, .. } if id == target_id => {
                // This shouldn't happen directly if called correctly from Split, 
                // but if the root is a Pane, we handle it in the caller.
                false
            }
            LayoutNode::Pane { .. } => false,
            LayoutNode::Split { axis: current_axis, children, weights, .. } => {
                let mut found_idx = None;
                for (i, child) in children.iter_mut().enumerate() {
                    if let LayoutNode::Pane { id, .. } = child {
                        if id == target_id {
                            found_idx = Some(i);
                            break;
                        }
                    } else {
                        if perform_split(child, target_id, new_node.clone(), axis.clone(), after) {
                            return true;
                        }
                    }
                }

                if let Some(idx) = found_idx {
                    if *current_axis == axis {
                        let insert_at = if after { idx + 1 } else { idx };
                        children.insert(insert_at, new_node);
                        if !weights.is_empty() {
                            weights.insert(insert_at, 1.0);
                        }
                    } else {
                        let old_child = children.remove(idx);
                        let nested_children = if after {
                            vec![old_child, new_node]
                        } else {
                            vec![new_node, old_child]
                        };
                        children.insert(idx, LayoutNode::Split {
                            id: format!("split-{}", Uuid::new_v4().to_string().get(..8).unwrap_or("sub")),
                            axis,
                            children: nested_children,
                            weights: vec![1.0, 1.0],
                        });
                    }
                    return true;
                }
                false
            }
        }
    }

    let split_done = if let LayoutNode::Pane { id, .. } = &session.root {
        if id == &target_panel_id {
            let old_root = session.root.clone();
            let children = if after {
                vec![old_root, new_pane]
            } else {
                vec![new_pane, old_root]
            };
            session.root = LayoutNode::Split {
                id: "root-split".to_string(),
                axis,
                children,
                weights: vec![1.0, 1.0],
            };
            true
        } else { false }
    } else {
        perform_split(&mut session.root, &target_panel_id, new_pane, axis, after)
    };

    if !split_done {
        return Err(CommandError::Other("Could not find target pane for split".to_string()));
    }

    // 5. Cleanup empty source
    if source_became_empty {
        let (removed, replacement) = session.root.remove_pane(&source_panel_id);
        if removed {
            // Root was the pane
            return Err(CommandError::Other("Cannot remove last pane".into()));
        }
        if let Some(r) = replacement {
            session.root = r;
        }
    }

    session.active_panel_id = new_panel_id.clone();
    if let Some(p) = session.root.find_pane_mut(&new_panel_id) {
        p.update_watcher(&app);
    }

    app.emit("session_changed", session.clone()).map_err(|e| CommandError::SystemError(e.to_string()))?;
    drop(session);
    state.save(&app)?;
    Ok(())
}

#[tauri::command]
pub fn remove_panel(
    app: AppHandle,
    state: State<'_, SessionManager>,
    panel_id: String,
) -> Result<(), CommandError> {
    let mut session = lock_session(&state)?;
    
    let all_ids = session.root.all_pane_ids();
    if all_ids.len() <= 1 {
        return Err(CommandError::Other("Cannot remove the last panel".to_string()));
    }

    let (is_root, replacement) = session.root.remove_pane(&panel_id);
    if is_root {
         return Err(CommandError::Other("Cannot remove root pane if it is the only one".to_string()));
    }
    if let Some(r) = replacement {
        session.root = r;
    }

    if session.active_panel_id == panel_id {
        if let Some(first) = session.root.all_pane_ids().first() {
            session.active_panel_id = first.clone();
        }
    }

    app.emit("session_changed", session.clone()).map_err(|e| CommandError::SystemError(e.to_string()))?;
    drop(session);
    state.save(&app)?;
    Ok(())
}

#[tauri::command]
pub fn update_sort_config(
    app: AppHandle,
    state: State<'_, SessionManager>,
    panel_id: String,
    sort_config: crate::models::session::SortConfig,
) -> Result<(), CommandError> {
    let mut session = lock_session(&state)?;
    
    let panel = session.try_get_panel_mut(&panel_id)?;
    panel.sort_config = sort_config;
    panel.update_watcher(&app);
    
    app.emit("session_changed", session.clone()).map_err(|e| CommandError::SystemError(e.to_string()))?;
    drop(session);
    state.save(&app)?;
    Ok(())
}

#[tauri::command]
pub fn add_panel(
    app: AppHandle,
    state: State<'_, SessionManager>,
    panel_id: Option<String>,
    path: Option<String>,
) -> Result<(), CommandError> {
    let mut session = lock_session(&state)?;
    
    let actual_id = panel_id.unwrap_or_else(|| {
        format!("panel-{}", Uuid::new_v4().to_string().get(..8).unwrap_or("extra"))
    });

    let default_tab_id = Uuid::new_v4().to_string();
    let mut panel = PanelState {
        tabs: vec![Tab {
            id: default_tab_id.clone(),
            path: PathBuf::from(path.unwrap_or_else(|| "C:\\".to_string())),
            version: 0,
        }],
        active_tab_id: default_tab_id,
        watcher: None,
        watched_path: None,
        search_context: None,
        sort_config: SortConfig::default(),
        cached_results: None,
    };
    panel.update_watcher(&app);

    let new_node = LayoutNode::Pane { id: actual_id.clone(), state: panel };

    match &mut session.root {
        LayoutNode::Pane { .. } => {
            let old_root = session.root.clone();
            session.root = LayoutNode::Split {
                id: "root-split".to_string(),
                axis: LayoutAxis::Horizontal,
                children: vec![old_root, new_node],
                weights: vec![1.0, 1.0],
            };
        }
        LayoutNode::Split { children, weights, .. } => {
            children.push(new_node);
            if !weights.is_empty() {
                weights.push(1.0);
            }
        }
    }

    session.active_panel_id = actual_id;

    app.emit("session_changed", session.clone()).map_err(|e| CommandError::SystemError(e.to_string()))?;
    drop(session);
    state.save(&app)?;
    Ok(())
}

#[tauri::command]
pub fn move_tab_between_panels(
    app: AppHandle,
    state: State<'_, SessionManager>,
    tab_id: String,
    target_panel_id: String,
    target_index: usize,
) -> Result<(), CommandError> {
    let mut session = lock_session(&state)?;
    
    let mut moved_tab = None;
    let mut source_panel_id = None;

    // Find source
    for pane_id in session.root.all_pane_ids() {
        let p = session.try_get_panel_mut(&pane_id)?;
        if let Some(pos) = p.tabs.iter().position(|t| t.id == tab_id) {
            source_panel_id = Some(pane_id.clone());
            moved_tab = Some(p.tabs.remove(pos));
            
            if p.tabs.is_empty() {
                // Last tab moved out — pane will be removed after the move
            } else if p.active_tab_id == tab_id {
                let new_pos = pos.min(p.tabs.len().saturating_sub(1));
                p.active_tab_id = p.tabs[new_pos].id.clone();
            }
            p.update_watcher(&app);
            break;
        }
    }

    let source_id = source_panel_id.ok_or_else(|| CommandError::Other("Source tab not found".into()))?;
    if source_id == target_panel_id { return Ok(()); }

    if let Some(tab) = moved_tab {
        let target = session.try_get_panel_mut(&target_panel_id)?;
        let idx = target_index.min(target.tabs.len());
        target.tabs.insert(idx, tab.clone());
        target.active_tab_id = tab.id;
        target.update_watcher(&app);
        session.active_panel_id = target_panel_id.clone();
    }

    // Remove the source pane if it is now empty and it's not the only pane
    let source_pane_empty = session.try_get_panel_mut(&source_id)?.tabs.is_empty();
    if source_pane_empty && session.root.all_pane_ids().len() > 1 {
        let (_, replacement) = session.root.remove_pane(&source_id);
        if let Some(r) = replacement {
            session.root = r;
        }
        if session.active_panel_id == source_id {
            if let Some(first) = session.root.all_pane_ids().first() {
                session.active_panel_id = first.clone();
            }
        }
    }

    app.emit("session_changed", session.clone()).map_err(|e| CommandError::SystemError(e.to_string()))?;
    drop(session);
    state.save(&app)?;
    Ok(())
}

#[tauri::command]
pub fn swap_panels(
    _app: AppHandle,
    _state: State<'_, SessionManager>,
) -> Result<(), CommandError> {
    // In a recursive tree, "swap" is ambiguous. 
    // We'll leave this as a no-op or error for now as it's a legacy dual-pane command.
    Err(CommandError::Other("Swap is no longer supported in multipane tree mode. Use drag and drop.".into()))
}

#[tauri::command]
pub fn update_layout_weights(
    app: AppHandle,
    state: State<'_, SessionManager>,
    split_id: String,
    weights: Vec<f32>,
) -> Result<(), CommandError> {
    let mut session = lock_session(&state)?;
    
    fn update_recursive(node: &mut LayoutNode, target_id: &str, new_weights: &[f32]) -> bool {
        match node {
            LayoutNode::Pane { .. } => false,
            LayoutNode::Split { id, weights, children, .. } => {
                if id == target_id {
                    if new_weights.len() == children.len() {
                        *weights = new_weights.to_vec();
                        return true;
                    }
                    return false;
                }
                for child in children {
                    if update_recursive(child, target_id, new_weights) {
                        return true;
                    }
                }
                false
            }
        }
    }

    if update_recursive(&mut session.root, &split_id, &weights) {
        app.emit("session_changed", session.clone()).map_err(|e| CommandError::SystemError(e.to_string()))?;
        drop(session);
        state.save(&app)?;
        Ok(())
    } else {
        Err(CommandError::Other("Split node not found".to_string()))
    }
}

#[tauri::command]
pub fn reorder_tabs(
    app: AppHandle,
    state: State<'_, SessionManager>,
    panel_id: String,
    source_index: usize,
    target_index: usize,
) -> Result<(), CommandError> {
    let mut session = lock_session(&state)?;
    let panel = session.try_get_panel_mut(&panel_id)?;

    if source_index < panel.tabs.len() && target_index <= panel.tabs.len() {
        let tab = panel.tabs.remove(source_index);
        let final_index = if target_index > source_index {
            target_index - 1
        } else {
            target_index
        };
        panel.tabs.insert(final_index, tab);
        panel.update_watcher(&app);
    }

    app.emit("session_changed", session.clone()).map_err(|e| CommandError::SystemError(e.to_string()))?;
    drop(session);
    state.save(&app)?;
    Ok(())
}
