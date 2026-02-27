use serde::{Deserialize, Serialize};

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
struct FsChangeEvent {
    kind: String,
    paths: Vec<String>,
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
        if path_str.starts_with("trash://") || path_str.starts_with("search://") {
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
                        .map(|p| p.to_string_lossy().to_string())
                        .collect();
                    
                    let _ = app_handle.emit("fs-change", FsChangeEvent { kind, paths });
                },
                Err(e) => log::error!("Watch error: {:?}", e),
            },
            Config::default(),
        ) {
            Ok(mut watcher) => {
                if let Err(e) = watcher.watch(&active_path, RecursiveMode::NonRecursive) {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub left_panel: PanelState,
    pub right_panel: PanelState,
    pub active_panel: String, // "left" or "right"
}

impl SessionState {
    /// Get a mutable reference to the panel identified by `id` ("left" or "right").
    pub fn get_panel_mut(&mut self, id: &str) -> &mut PanelState {
        if id == "left" { &mut self.left_panel } else { &mut self.right_panel }
    }
}

impl Default for SessionState {
    fn default() -> Self {
        SessionState {
            left_panel: PanelState {
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
            },
            right_panel: PanelState {
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
            },
            active_panel: "left".to_string(),
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
        let config_dir = app_handle.path().app_config_dir().map_err(|e: tauri::Error| CommandError::IoError(e.to_string()))?;
        
        if !config_dir.exists() {
            fs::create_dir_all(&config_dir).map_err(|e| CommandError::IoError(e.to_string()))?;
        }
        
        let session_path = config_dir.join("session.json");
        let json = serde_json::to_string_pretty(&*session).map_err(|e| CommandError::Other(e.to_string()))?;
        
        fs::write(session_path, json).map_err(|e| CommandError::IoError(e.to_string()))?;
        Ok(())
    }

    pub fn load(&self, app_handle: &AppHandle) -> Result<(), CommandError> {
        let config_dir = app_handle.path().app_config_dir().map_err(|e: tauri::Error| CommandError::IoError(e.to_string()))?;
        let session_path = config_dir.join("session.json");

        if session_path.exists() {
            let content = fs::read_to_string(session_path).map_err(|e| CommandError::IoError(e.to_string()))?;
            match serde_json::from_str::<SessionState>(&content) {
                Ok(mut loaded_session) => {
                    // Update watchers for the loaded paths
                    loaded_session.left_panel.update_watcher(app_handle);
                    loaded_session.right_panel.update_watcher(app_handle);

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
