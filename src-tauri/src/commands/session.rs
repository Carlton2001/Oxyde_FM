use crate::models::{CommandError, SessionManager, SessionState, Tab};
use tauri::{AppHandle, Emitter, State};
use std::path::PathBuf;
use std::sync::MutexGuard;
use uuid::Uuid;

/// R1: Helper to lock session state, eliminating repeated map_err boilerplate.
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

    let panel = session.get_panel_mut(&panel_id);
    panel.tabs.push(new_tab);
    
    // Only switch if not background
    if !background.unwrap_or(false) {
        panel.active_tab_id = new_id.clone();
        // Auto-switch focus to this panel
        session.active_panel_id = panel_id.clone();
    }

    session.get_panel_mut(&panel_id).update_watcher(&app);

    app.emit("session_changed", session.clone()).map_err(|e| CommandError::SystemError(e.to_string()))?;
    drop(session);
    state.save(&app)?;
    
    Ok(new_id)
}

#[tauri::command]
pub fn close_tab(
    app: AppHandle,
    state: State<'_, SessionManager>,
    tab_id: String,
) -> Result<(), CommandError> {
    let mut session = lock_session(&state)?;
    
    for panel in session.panels.values_mut() {
        if let Some(pos) = panel.tabs.iter().position(|t| t.id == tab_id) {
            panel.tabs.remove(pos);
            // If we closed the active tab, switch to the nearest one (or create default)
            if panel.active_tab_id == tab_id {
                let new_pos = pos.min(panel.tabs.len().saturating_sub(1));
                if let Some(next_tab) = panel.tabs.get(new_pos) {
                    panel.active_tab_id = next_tab.id.clone();
                } else {
                    // Create a default tab if all closed
                    let default_id = Uuid::new_v4().to_string();
                    panel.tabs.push(Tab {
                        id: default_id.clone(),
                        path: PathBuf::from("C:\\"),
                        version: 0,
                    });
                    panel.active_tab_id = default_id;
                }
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
pub fn switch_tab(
    app: AppHandle,
    state: State<'_, SessionManager>,
    tab_id: String,
) -> Result<(), CommandError> {
    let mut session = lock_session(&state)?;

    let mut found_panel_id = None;
    for (id, panel) in &mut session.panels {
        if panel.tabs.iter().any(|t| t.id == tab_id) {
            panel.active_tab_id = tab_id.clone();
            found_panel_id = Some(id.clone());
        }
        panel.update_watcher(&app);
    }

    if let Some(id) = found_panel_id {
        session.active_panel_id = id;
    } else {
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
        let panel = session.get_panel_mut(&panel_id);
        if let Some(tab) = panel.tabs.iter_mut().find(|t| t.id == panel.active_tab_id) {
            // Only update if the incoming version is newer or if no version is provided (legacy/internal)
            if let Some(v) = version {
                if v > tab.version {
                    tab.path = PathBuf::from(&path);
                    tab.version = v;
                } else if v < tab.version {
                    // log::warn!("REJECTED: Navigation to {:?} (v{}) because current is v{}", path, v, tab.version);
                    return Ok(());
                } else if tab.path.to_string_lossy() != path {
                    tab.path = PathBuf::from(&path);
                }
            } else {
                tab.path = PathBuf::from(path);
                tab.version += 1;
            }
        }
    }

    // Update watcher for the affected panel
    session.get_panel_mut(&panel_id).update_watcher(&app);
    
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
    
    if !session.panels.contains_key(&panel_id) {
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

    for panel in session.panels.values_mut() {
        if let Some(pos) = panel.tabs.iter().position(|t| t.id == tab_id) {
            let tab = &panel.tabs[pos];
            let new_tab = Tab {
                id: Uuid::new_v4().to_string(),
                path: tab.path.clone(),
                version: tab.version,
            };
            panel.tabs.insert(pos + 1, new_tab.clone());
            panel.active_tab_id = new_tab.id;
        }
        panel.update_watcher(&app);
    }

    app.emit("session_changed", session.clone()).map_err(|e| CommandError::SystemError(e.to_string()))?;
    drop(session);
    state.save(&app)?;
    Ok(())
}

#[tauri::command]
pub fn close_other_tabs(
    app: AppHandle,
    state: State<'_, SessionManager>,
    tab_id: String,
) -> Result<(), CommandError> {
    let mut session = lock_session(&state)?;

    for panel in session.panels.values_mut() {
        if let Some(target_tab) = panel.tabs.iter().find(|t| t.id == tab_id).cloned() {
            panel.tabs = vec![target_tab];
            panel.active_tab_id = tab_id.clone();
        }
        panel.update_watcher(&app);
    }

    app.emit("session_changed", session.clone()).map_err(|e| CommandError::SystemError(e.to_string()))?;
    drop(session);
    state.save(&app)?;
    Ok(())
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

    let panel = session.get_panel_mut(&panel_id);

    if source_index < panel.tabs.len() && target_index <= panel.tabs.len() {
        let tab = panel.tabs.remove(source_index);
        let final_index = if target_index > source_index {
            target_index - 1
        } else {
            target_index
        };
        panel.tabs.insert(final_index, tab);
    } else {
        return Err(CommandError::Other("Index out of bounds".to_string()));
    }

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
    for (id, panel) in &session.panels {
        if panel.tabs.iter().any(|t| t.id == tab_id) {
            source_panel_id = Some(id.clone());
            break;
        }
    }

    let source_panel_id = source_panel_id.ok_or_else(|| CommandError::Other("Source panel not found".to_string()))?;
    
    if source_panel_id == target_panel_id {
        // This should normally be handled by reorder_tabs on the frontend, 
        // but if it hits here, we just return.
        return Ok(());
    }
    
    {
        let source_panel = session.get_panel_mut(&source_panel_id);
        if let Some(pos) = source_panel.tabs.iter().position(|t| t.id == tab_id) {
            moved_tab = Some(source_panel.tabs.remove(pos));
            
            // If we closed the active tab in source, switch to another one
            if source_panel.active_tab_id == tab_id {
                let new_pos = pos.min(source_panel.tabs.len().saturating_sub(1));
                if let Some(next_tab) = source_panel.tabs.get(new_pos) {
                    source_panel.active_tab_id = next_tab.id.clone();
                } else {
                    // Create a default tab if all closed
                    let default_id = uuid::Uuid::new_v4().to_string();
                    source_panel.tabs.push(Tab {
                        id: default_id.clone(),
                        path: std::path::PathBuf::from("C:\\"),
                        version: 0,
                    });
                    source_panel.active_tab_id = default_id;
                }
            }
        }
    }
    
    if let Some(tab) = moved_tab {
        let target_panel = session.get_panel_mut(&target_panel_id);
        let idx = target_index.min(target_panel.tabs.len());
        target_panel.tabs.insert(idx, tab.clone());
        target_panel.active_tab_id = tab.id;
        
        // Auto-focus the target panel
        session.active_panel_id = target_panel_id;
    }

    // Refresh watchers
    for panel in session.panels.values_mut() {
        panel.update_watcher(&app);
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
    
    session.get_panel_mut(&panel_id).sort_config = sort_config;
    
    app.emit("session_changed", session.clone()).map_err(|e| CommandError::SystemError(e.to_string()))?;
    drop(session);
    state.save(&app)?;
    Ok(())
}
#[tauri::command]
pub fn swap_panels(
    app: AppHandle,
    state: State<'_, SessionManager>,
) -> Result<(), CommandError> {
    let mut session_guard = lock_session(&state)?;
    let session = &mut *session_guard;
    
    // Only allow swapping if we have "left" and "right" panels (legacy support)
    // In a multi-pane world, "swap" might need different arguments, but for now we keep the dual logic.
    let (l_exists, r_exists) = {
        (session.panels.contains_key("left"), session.panels.contains_key("right"))
    };

    if !l_exists || !r_exists {
        return Err(CommandError::Other("Swap requires both 'left' and 'right' panels".to_string()));
    }

    let mut left_panel = session.panels.shift_remove("left").unwrap();
    let mut right_panel = session.panels.shift_remove("right").unwrap();

    // 1. Identify active tabs on both sides
    let l_active_id = left_panel.active_tab_id.clone();
    let r_active_id = right_panel.active_tab_id.clone();

    // 2. Perform the content exchange between active tabs
    {
        let l_tab = left_panel.tabs.iter_mut().find(|t| t.id == l_active_id);
        let r_tab = right_panel.tabs.iter_mut().find(|t| t.id == r_active_id);

        if let (Some(lt), Some(rt)) = (l_tab, r_tab) {
            std::mem::swap(&mut lt.path, &mut rt.path);
            std::mem::swap(&mut lt.version, &mut rt.version);
        }
    }

    // 3. Swap panel-level states (search, sort, cache)
    std::mem::swap(&mut left_panel.search_context, &mut right_panel.search_context);
    std::mem::swap(&mut left_panel.sort_config, &mut right_panel.sort_config);
    std::mem::swap(&mut left_panel.cached_results, &mut right_panel.cached_results);

    // 4. Update tab_ids in search contexts
    if let Some(ctx) = &mut left_panel.search_context { ctx.tab_id = l_active_id; }
    if let Some(ctx) = &mut right_panel.search_context { ctx.tab_id = r_active_id; }

    // 5. Update watchers
    left_panel.update_watcher(&app);
    right_panel.update_watcher(&app);

    // Put them back
    session.panels.insert("left".to_string(), left_panel);
    session.panels.insert("right".to_string(), right_panel);
    
    app.emit("session_changed", session.clone()).map_err(|e| CommandError::SystemError(e.to_string()))?;
    drop(session_guard);
    state.save(&app)?;
    Ok(())
}
