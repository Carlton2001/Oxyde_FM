use serde_json;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize, Deserializer};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tab {
    pub id: String,
    pub path: PathBuf,
    pub version: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelState {
    pub tabs: Vec<Tab>,
    pub active_tab_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionState {
    pub panels: IndexMap<String, PanelState>,
    pub active_panel_id: String,
}

impl<'de> Deserialize<'de> for SessionState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct LegacySession {
            #[serde(default)]
            left_panel: Option<PanelState>,
            #[serde(default)]
            right_panel: Option<PanelState>,
            #[serde(default)]
            active_panel: Option<String>,
            #[serde(default)]
            panels: Option<IndexMap<String, PanelState>>,
            #[serde(default)]
            active_panel_id: Option<String>,
        }

        let legacy = LegacySession::deserialize(deserializer)?;
        let mut panels = legacy.panels.unwrap_or_default();
        let mut active_panel_id = legacy.active_panel_id.unwrap_or_else(|| "left".to_string());

        if let Some(left) = legacy.left_panel {
            if !panels.contains_key("left") {
                panels.insert("left".to_string(), left);
            }
        }
        if let Some(right) = legacy.right_panel {
            if !panels.contains_key("right") {
                panels.insert("right".to_string(), right);
            }
        }
        if let Some(old_active) = legacy.active_panel {
             active_panel_id = old_active;
        }

        Ok(SessionState { panels, active_panel_id })
    }
}

fn main() {
    let legacy_json = r#"{
        "left_panel": { "tabs": [], "active_tab_id": "t1" },
        "right_panel": { "tabs": [], "active_tab_id": "t2" },
        "active_panel": "right"
    }"#;

    let session: SessionState = serde_json::from_str(legacy_json).unwrap();
    println!("Migrated Panels: {:?}", session.panels.keys());
    println!("Active Panel: {}", session.active_panel_id);
    
    assert!(session.panels.contains_key("left"));
    assert!(session.panels.contains_key("right"));
    assert_eq!(session.active_panel_id, "right");
    println!("Test Passed!");
}
