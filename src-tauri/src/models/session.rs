use serde::{Deserialize, Serialize, Deserializer};
use indexmap::IndexMap;

use std::path::PathBuf;
use std::sync::Mutex;

use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use std::fs;
use crate::models::file_entry::FileEntry;
use crate::models::CommandError;
use std::sync::atomic::AtomicBool;

#[derive(Clone, Serialize)]
pub struct FsChangeEvent {
    pub kind: String,
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum SortField {
    Name,
    Size,
    Date,
    Type,
    Location,
    DeletedDate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum SortDirection {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SortConfig {
    pub field: SortField,
    pub direction: SortDirection,
}

impl Default for SortConfig {
    fn default() -> Self {
        Self {
            field: SortField::Name,
            direction: SortDirection::Asc,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tab {
    pub id: String,
    pub path: PathBuf,
    #[serde(default)]
    pub version: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchContext {
    pub tab_id: String,
    pub query: String,
    #[serde(skip)] // Do NOT send search results via the global session state
    pub results: Vec<FileEntry>,
    pub is_searching: bool,
    #[serde(skip)]
    pub cancellation_token: Option<Arc<AtomicBool>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedResults {
    pub path: PathBuf,
    #[serde(skip)] // Do NOT send cached entries via session state
    pub entries: Vec<FileEntry>, 
    pub summary: crate::models::FileSummary,
    pub config: SortConfig,
    pub show_hidden: bool,
    pub show_system: bool,
    pub is_protected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelState {
    pub tabs: Vec<Tab>,
    pub active_tab_id: String,
    #[serde(skip)]
    pub watcher: Option<Arc<Mutex<RecommendedWatcher>>>,
    #[serde(skip)]
    pub watched_path: Option<PathBuf>,
    #[serde(default)]
    pub search_context: Option<SearchContext>,
    #[serde(default)]
    pub sort_config: SortConfig,
    #[serde(default)]
    pub cached_results: Option<CachedResults>,
}

impl PanelState {
    pub fn update_watcher(&mut self, app_handle: &AppHandle) {
        let active_path = self.tabs.iter()
            .find(|t| t.id == self.active_tab_id)
            .map(|t| t.path.clone())
            .unwrap_or_else(|| PathBuf::from("C:\\"));

        // Skip watching virtual paths (like trash:// or search://)
        let path_str = active_path.to_string_lossy().to_lowercase();
        let path_str = path_str.replace('\\', "/");
        if path_str.starts_with("trash://") || path_str.starts_with("search://") || path_str == "__network_vincinity__" {
            self.watcher = None;
            self.watched_path = None;
            return;
        }

        // Skip recreation if already watching the same path
        if self.watched_path.as_ref() == Some(&active_path) && self.watcher.is_some() {
            return;
        }
        
        let app_handle = app_handle.clone();

        match RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| match res {
                Ok(event) => {
                    let kind = format!("{:?}", event.kind);
                    if kind.contains("Access") { return; } // Filter noisy events

                    let paths: Vec<String> = event.paths.iter()
                        .map(|p| p.to_string_lossy().replace("\\\\?\\", "").to_string())
                        .collect();
                    
                    let _ = app_handle.emit("fs-change", FsChangeEvent { kind, paths });
                },
                Err(e) => log::error!("Watch error: {:?}", e),
            },
            Config::default(),
        ) {
            Ok(mut watcher) => {
                if let Err(e) = watcher.watch(&active_path, RecursiveMode::Recursive) {
                    // Don't log as ERROR for things we might not have access to (system folders)
                    log::warn!("Could not watch {:?} (Protected or Virtual): {}", active_path, e);
                } else {
                    self.watched_path = Some(active_path);
                    self.watcher = Some(Arc::new(Mutex::new(watcher)));
                }
            },
            Err(e) => log::error!("Failed to create watcher: {}", e),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum LayoutAxis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
#[allow(clippy::large_enum_variant)]
pub enum LayoutNode {
    Pane {
        id: String,
        state: PanelState,
    },
    Split {
        id: String,
        axis: LayoutAxis,
        children: Vec<LayoutNode>,
        #[serde(default)]
        weights: Vec<f32>,
    },
}

impl LayoutNode {
    pub fn find_pane(&self, target_id: &str) -> Option<&PanelState> {
        match self {
            LayoutNode::Pane { id, state } => {
                if id == target_id {
                    Some(state)
                } else {
                    None
                }
            }
            LayoutNode::Split { children, .. } => {
                for child in children {
                    if let Some(state) = child.find_pane(target_id) {
                        return Some(state);
                    }
                }
                None
            }
        }
    }

    pub fn find_pane_mut(&mut self, target_id: &str) -> Option<&mut PanelState> {
        match self {
            LayoutNode::Pane { id, state } => {
                if id == target_id {
                    Some(state)
                } else {
                    None
                }
            }
            LayoutNode::Split { children, .. } => {
                for child in children {
                    if let Some(state) = child.find_pane_mut(target_id) {
                        return Some(state);
                    }
                }
                None
            }
        }
    }

    pub fn remove_pane(&mut self, target_id: &str) -> (bool, Option<LayoutNode>) {
        match self {
            LayoutNode::Pane { id, .. } => {
                if id == target_id {
                    (true, None)
                } else {
                    (false, None)
                }
            }
            LayoutNode::Split { children, weights, .. } => {
                let mut update_at = None;

                for (i, child) in children.iter_mut().enumerate() {
                    let (found, replacement) = child.remove_pane(target_id);
                    if found || replacement.is_some() {
                        update_at = Some((i, replacement));
                        break;
                    }
                }

                if let Some((idx, replacement)) = update_at {
                    if let Some(r) = replacement {
                        children[idx] = r;
                    } else {
                        children.remove(idx);
                        if !weights.is_empty() && idx < weights.len() {
                            weights.remove(idx);
                        }
                    }

                    // Collapse: if only 1 child remains, this Split is no longer needed
                    if children.len() == 1 {
                        return (false, Some(children.remove(0)));
                    }
                    (false, None)
                } else {
                    (false, None)
                }
            }
        }
    }

    pub fn all_pane_ids(&self) -> Vec<String> {
        match self {
            LayoutNode::Pane { id, .. } => vec![id.clone()],
            LayoutNode::Split { children, .. } => {
                children.iter().flat_map(|c| c.all_pane_ids()).collect()
            }
        }
    }

    pub fn all_panels(&self) -> Vec<&PanelState> {
        match self {
            LayoutNode::Pane { state, .. } => vec![state],
            LayoutNode::Split { children, .. } => {
                children.iter().flat_map(|c| c.all_panels()).collect()
            }
        }
    }

    pub fn all_panels_mut(&mut self) -> Vec<&mut PanelState> {
        match self {
            LayoutNode::Pane { state, .. } => vec![state],
            LayoutNode::Split { children, .. } => {
                children.iter_mut().flat_map(|c| c.all_panels_mut()).collect()
            }
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionState {
    pub root: LayoutNode,
    pub active_panel_id: String,
}

// Custom deserialization for migration "sans tout casser"
impl<'de> Deserialize<'de> for SessionState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct LegacySession {
            #[serde(default)]
            panels: Option<IndexMap<String, PanelState>>,
            #[serde(default)]
            active_panel_id: Option<String>,
            #[serde(default)]
            root: Option<LayoutNode>,
        }

        let legacy = LegacySession::deserialize(deserializer)?;
        
        if let Some(root) = legacy.root {
            return Ok(SessionState {
                root,
                active_panel_id: legacy.active_panel_id.unwrap_or_else(|| "left".to_string()),
            });
        }

        let panels = legacy.panels.unwrap_or_default();
        let active_panel_id = legacy.active_panel_id.unwrap_or_else(|| "left".to_string());

        if panels.is_empty() {
            return Ok(SessionState::default());
        }

        // Migrate flat map to a horizontal split
        let children: Vec<LayoutNode> = panels.into_iter()
            .map(|(id, state)| LayoutNode::Pane { id, state })
            .collect();

        let root = if children.len() == 1 {
            children.into_iter().next().unwrap()
        } else {
            let count = children.len();
            LayoutNode::Split {
                id: "root-split".to_string(),
                axis: LayoutAxis::Horizontal,
                children,
                weights: vec![1.0; count],
            }
        };

        Ok(SessionState {
            root,
            active_panel_id,
        })
    }
}

impl SessionState {
    pub fn get_panel(&self, id: &str) -> &PanelState {
        self.root.find_pane(id).unwrap_or_else(|| panic!("Panel '{}' not found in session", id))
    }

    pub fn get_panel_mut(&mut self, id: &str) -> &mut PanelState {
        self.root.find_pane_mut(id).unwrap_or_else(|| panic!("Panel '{}' not found in session", id))
    }

    pub fn try_get_panel(&self, id: &str) -> Result<&PanelState, CommandError> {
        self.root.find_pane(id).ok_or_else(|| CommandError::Other(format!("Panel '{}' not found", id)))
    }

    pub fn try_get_panel_mut(&mut self, id: &str) -> Result<&mut PanelState, CommandError> {
        self.root.find_pane_mut(id).ok_or_else(|| CommandError::Other(format!("Panel '{}' not found", id)))
    }
}

impl Default for SessionState {
    fn default() -> Self {
        let left_id = "left".to_string();
        let right_id = "right".to_string();

        let left_panel = PanelState {
            tabs: vec![Tab {
                id: "default-left".to_string(),
                path: PathBuf::from("C:\\"),
                version: 0,
            }],
            active_tab_id: "default-left".to_string(),
            watcher: None,
            watched_path: None,
            search_context: None,
            sort_config: SortConfig::default(),
            cached_results: None,
        };

        let right_panel = PanelState {
            tabs: vec![Tab {
                id: "default-right".to_string(),
                path: PathBuf::from("C:\\"),
                version: 0,
            }],
            active_tab_id: "default-right".to_string(),
            watcher: None,
            watched_path: None,
            search_context: None,
            sort_config: SortConfig::default(),
            cached_results: None,
        };

        let root = LayoutNode::Split {
            id: "root-split".to_string(),
            axis: LayoutAxis::Horizontal,
            children: vec![
                LayoutNode::Pane { id: left_id.clone(), state: left_panel },
                LayoutNode::Pane { id: right_id, state: right_panel },
            ],
            weights: vec![1.0, 1.0],
        };

        SessionState {
            root,
            active_panel_id: left_id,
        }
    }
}

pub struct SessionManager(pub Mutex<SessionState>);

impl Default for SessionManager {
    fn default() -> Self {
        Self(Mutex::new(SessionState::default()))
    }
}

impl SessionManager {
    pub fn save(&self, app_handle: &AppHandle) -> Result<(), CommandError> {
        let session = self.0.lock().map_err(|_| CommandError::SystemError("Failed to lock session state".to_string()))?;
        let config_dir = app_handle.path().app_local_data_dir().map_err(|e: tauri::Error| CommandError::IoError(e.to_string()))?;
        
        if !config_dir.exists() {
            fs::create_dir_all(&config_dir).map_err(|e| CommandError::IoError(e.to_string()))?;
        }
        
        let session_path = config_dir.join("session.json");
        let json = serde_json::to_string_pretty(&*session).map_err(|e| CommandError::Other(e.to_string()))?;
        
        fs::write(session_path, json).map_err(|e| CommandError::IoError(e.to_string()))?;
        Ok(())
    }

    pub fn load(&self, app_handle: &AppHandle) -> Result<(), CommandError> {
        let config_dir = app_handle.path().app_local_data_dir().map_err(|e: tauri::Error| CommandError::IoError(e.to_string()))?;
        let session_path = config_dir.join("session.json");

        if session_path.exists() {
            let content = fs::read_to_string(session_path).map_err(|e| CommandError::IoError(e.to_string()))?;
            match serde_json::from_str::<SessionState>(&content) {
                Ok(mut loaded_session) => {
                    // Update watchers for all panels in the tree
                    for panel in loaded_session.root.all_panels_mut() {
                        panel.update_watcher(app_handle);
                    }

                    let mut session = self.0.lock().map_err(|_| CommandError::SystemError("Failed to lock session state".to_string()))?;
                    *session = loaded_session;
                    
                    // Emit immediately after load so UI knows the restored state
                    if let Err(e) = app_handle.emit("session_changed", session.clone()) {
                        log::error!("Failed to emit session after load: {}", e);
                    }
                },
                Err(e) => log::error!("Failed to parse session.json: {}", e),
            }
        }
        Ok(())
    }
}

pub struct SidebarWatcherState {
    pub watcher: Mutex<Option<RecommendedWatcher>>,
}

impl Default for SidebarWatcherState {
    fn default() -> Self {
        Self { watcher: Mutex::new(None) }
    }
}
