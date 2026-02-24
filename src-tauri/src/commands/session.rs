use crate::models::{CommandError, SessionManager, SessionState, Tab};
use crate::models::session::PanelState;
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
    };

    let panel = session.get_panel_mut(&panel_id);
    panel.tabs.push(new_tab);
    
    // Only switch if not background
    if !background.unwrap_or(false) {
        panel.active_tab_id = new_id.clone();
        // Auto-switch focus to this panel
        session.active_panel = panel_id.clone();
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
    
    // Helper to remove tab from a panel
    let remove_from_panel = |panel: &mut PanelState| -> bool {
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
                    });
                    panel.active_tab_id = default_id;
                }
            }
            true
        } else {
            false
        }
    };

    if !remove_from_panel(&mut session.left_panel) {
        remove_from_panel(&mut session.right_panel);
    }
    
    // Update watchers for both panels just in case (active tab might have changed)
    session.left_panel.update_watcher(&app);
    session.right_panel.update_watcher(&app);

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

    // Find which panel contains this tab
    if session.left_panel.tabs.iter().any(|t| t.id == tab_id) {
        session.left_panel.active_tab_id = tab_id;
        session.active_panel = "left".to_string();
    } else if session.right_panel.tabs.iter().any(|t| t.id == tab_id) {
        session.right_panel.active_tab_id = tab_id;
        session.active_panel = "right".to_string();
    } else {
        return Err(CommandError::Other("Tab not found".to_string()));
    }
    
    // Update watchers
    session.left_panel.update_watcher(&app);
    session.right_panel.update_watcher(&app);

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
) -> Result<(), CommandError> {
    let mut session = lock_session(&state)?;
    
    {
        let panel = session.get_panel_mut(&panel_id);
        if let Some(tab) = panel.tabs.iter_mut().find(|t| t.id == panel.active_tab_id) {
            tab.path = PathBuf::from(path);
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
    
    if panel_id != "left" && panel_id != "right" {
         return Err(CommandError::Other("Invalid panel ID".to_string()));
    }

    session.active_panel = panel_id;
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

    // Helper to duplicate in a panel
    let duplicate_in_panel = |panel: &mut PanelState| -> bool {
        if let Some(pos) = panel.tabs.iter().position(|t| t.id == tab_id) {
            let tab = &panel.tabs[pos];
            let new_tab = Tab {
                id: Uuid::new_v4().to_string(),
                path: tab.path.clone(),
            };
            // Insert after current
            panel.tabs.insert(pos + 1, new_tab.clone());
            // Switch to it (optional, but standard behavior)
            panel.active_tab_id = new_tab.id;
            true
        } else {
            false
        }
    };

    if !duplicate_in_panel(&mut session.left_panel) {
        duplicate_in_panel(&mut session.right_panel);
    }
    
    session.left_panel.update_watcher(&app);
    session.right_panel.update_watcher(&app);

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

    let handle_panel = |panel: &mut PanelState| -> bool {
        // Check if tab exists in this panel
        if let Some(target_tab) = panel.tabs.iter().find(|t| t.id == tab_id).cloned() {
            // Replace all tabs with just this one
            panel.tabs = vec![target_tab];
            panel.active_tab_id = tab_id.clone();
            true
        } else {
            false
        }
    };

    if !handle_panel(&mut session.left_panel) {
        handle_panel(&mut session.right_panel);
    }

    session.left_panel.update_watcher(&app);
    session.right_panel.update_watcher(&app);

    app.emit("session_changed", session.clone()).map_err(|e| CommandError::SystemError(e.to_string()))?;
    drop(session);
    state.save(&app)?;
    Ok(())
}

#[tauri::command]
pub fn reorder_tabs(
    app: AppHandle,
    state: State<'_, SessionManager>,
    source_index: usize,
    target_index: usize,
) -> Result<(), CommandError> {
    let mut session = lock_session(&state)?;

    // B3 fix: bounds-check both indices before modifying
    let active = session.active_panel.clone();
    let panel = session.get_panel_mut(&active);

    if source_index < panel.tabs.len() && target_index < panel.tabs.len() {
        let tab = panel.tabs.remove(source_index);
        panel.tabs.insert(target_index, tab);
    } else {
        return Err(CommandError::Other("Index out of bounds".to_string()));
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
