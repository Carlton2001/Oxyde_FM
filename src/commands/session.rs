use crate::models::session::PanelState;
use crate::models::{CommandError, SessionManager, SessionState, Tab};
use std::path::PathBuf;
use std::sync::MutexGuard;
use uuid::Uuid;

/// lock session state helper
fn lock_session(state: &SessionManager) -> Result<MutexGuard<'_, SessionState>, CommandError> {
    state
        .0
        .lock()
        .map_err(|_| CommandError::SystemError("Failed to lock session state".into()))
}

pub fn get_session_state(state: &SessionManager) -> Result<SessionState, CommandError> {
    let session = lock_session(state)?;
    Ok(session.clone())
}

pub fn create_tab(
    state: &SessionManager,
    panel_id: String,
    path: String,
    background: Option<bool>,
) -> Result<String, CommandError> {
    let mut session = lock_session(state)?;

    let new_id = Uuid::new_v4().to_string();
    let new_tab = Tab {
        id: new_id.clone(),
        path: PathBuf::from(&path),
        back_stack: Vec::new(),
        forward_stack: Vec::new(),
        version: 0,
    };

    let panel = session.get_panel_mut(&panel_id);
    panel.tabs.push(new_tab);

    if !background.unwrap_or(false) {
        panel.active_tab_id = new_id.clone();
        session.active_panel = panel_id.clone();
    }

    session.get_panel_mut(&panel_id).update_watcher();

    drop(session);
    state.save()?;

    Ok(new_id)
}

pub fn close_tab(state: &SessionManager, tab_id: String) -> Result<(), CommandError> {
    let mut session = lock_session(state)?;

    let remove_from_panel = |panel: &mut PanelState| -> bool {
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
                        back_stack: Vec::new(),
                        forward_stack: Vec::new(),
                        version: 0,
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

    session.left_panel.update_watcher();
    session.right_panel.update_watcher();

    drop(session);
    state.save()?;
    Ok(())
}

pub fn switch_tab(state: &SessionManager, tab_id: String) -> Result<(), CommandError> {
    let mut session = lock_session(state)?;

    if session.left_panel.tabs.iter().any(|t| t.id == tab_id) {
        session.left_panel.active_tab_id = tab_id;
        session.active_panel = "left".to_string();
    } else if session.right_panel.tabs.iter().any(|t| t.id == tab_id) {
        session.right_panel.active_tab_id = tab_id;
        session.active_panel = "right".to_string();
    } else {
        return Err(CommandError::Other("Tab not found".to_string()));
    }

    session.left_panel.update_watcher();
    session.right_panel.update_watcher();

    drop(session);
    state.save()?;
    Ok(())
}

pub fn active_tab_navigate(
    state: &SessionManager,
    panel_id: String,
    path: String,
    version: Option<u64>,
) -> Result<(), CommandError> {
    let mut session = lock_session(state)?;

    {
        let panel = session.get_panel_mut(&panel_id);
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
    }

    session.get_panel_mut(&panel_id).update_watcher();

    drop(session);
    state.save()?;
    Ok(())
}

pub fn set_active_panel(state: &SessionManager, panel_id: String) -> Result<(), CommandError> {
    let mut session = lock_session(state)?;

    if panel_id != "left" && panel_id != "right" {
        return Err(CommandError::Other("Invalid panel ID".to_string()));
    }

    session.active_panel = panel_id;
    drop(session);
    state.save()?;
    Ok(())
}

pub fn duplicate_tab(state: &SessionManager, tab_id: String) -> Result<(), CommandError> {
    let mut session = lock_session(state)?;

    let duplicate_in_panel = |panel: &mut PanelState| -> bool {
        if let Some(pos) = panel.tabs.iter().position(|t| t.id == tab_id) {
            let tab = &panel.tabs[pos];
            let new_tab = Tab {
                id: Uuid::new_v4().to_string(),
                path: tab.path.clone(),
                back_stack: tab.back_stack.clone(),
                forward_stack: tab.forward_stack.clone(),
                version: tab.version,
            };
            panel.tabs.insert(pos + 1, new_tab.clone());
            panel.active_tab_id = new_tab.id;
            true
        } else {
            false
        }
    };

    if !duplicate_in_panel(&mut session.left_panel) {
        duplicate_in_panel(&mut session.right_panel);
    }

    session.left_panel.update_watcher();
    session.right_panel.update_watcher();

    drop(session);
    state.save()?;
    Ok(())
}

pub fn close_other_tabs(state: &SessionManager, tab_id: String) -> Result<(), CommandError> {
    let mut session = lock_session(state)?;

    let handle_panel = |panel: &mut PanelState| -> bool {
        if let Some(target_tab) = panel.tabs.iter().find(|t| t.id == tab_id).cloned() {
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

    session.left_panel.update_watcher();
    session.right_panel.update_watcher();

    drop(session);
    state.save()?;
    Ok(())
}

pub fn reorder_tabs(
    state: &SessionManager,
    source_index: usize,
    target_index: usize,
) -> Result<(), CommandError> {
    let mut session = lock_session(state)?;

    let active = session.active_panel.clone();
    let panel = session.get_panel_mut(&active);

    if source_index < panel.tabs.len() && target_index < panel.tabs.len() {
        let tab = panel.tabs.remove(source_index);
        panel.tabs.insert(target_index, tab);
    } else {
        return Err(CommandError::Other("Index out of bounds".to_string()));
    }

    drop(session);
    state.save()?;
    Ok(())
}

pub fn update_sort_config(
    state: &SessionManager,
    panel_id: String,
    sort_config: crate::models::session::SortConfig,
) -> Result<(), CommandError> {
    let mut session = lock_session(state)?;

    session.get_panel_mut(&panel_id).sort_config = sort_config;

    drop(session);
    state.save()?;
    Ok(())
}

pub fn swap_panels(state: &SessionManager) -> Result<(), CommandError> {
    let mut session_guard = lock_session(state)?;
    let session = &mut *session_guard;

    let l_active_id = session.left_panel.active_tab_id.clone();
    let r_active_id = session.right_panel.active_tab_id.clone();

    {
        let l_tab = session
            .left_panel
            .tabs
            .iter_mut()
            .find(|t| t.id == l_active_id);
        let r_tab = session
            .right_panel
            .tabs
            .iter_mut()
            .find(|t| t.id == r_active_id);

        if let (Some(lt), Some(rt)) = (l_tab, r_tab) {
            std::mem::swap(&mut lt.path, &mut rt.path);
            std::mem::swap(&mut lt.version, &mut rt.version);
        }
    }

    std::mem::swap(
        &mut session.left_panel.search_context,
        &mut session.right_panel.search_context,
    );
    std::mem::swap(
        &mut session.left_panel.sort_config,
        &mut session.right_panel.sort_config,
    );
    std::mem::swap(
        &mut session.left_panel.cached_results,
        &mut session.right_panel.cached_results,
    );

    if let Some(ctx) = &mut session.left_panel.search_context {
        ctx.tab_id = l_active_id;
    }
    if let Some(ctx) = &mut session.right_panel.search_context {
        ctx.tab_id = r_active_id;
    }

    session.left_panel.update_watcher();
    session.right_panel.update_watcher();

    drop(session_guard);
    state.save()?;
    Ok(())
}
