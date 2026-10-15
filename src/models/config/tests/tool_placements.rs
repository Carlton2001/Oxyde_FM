use super::{AppConfig, WindowPlacement};

#[test]
fn tool_positions_round_trip_and_survive_preference_reset() {
    let mut config = AppConfig::default();
    config.tool_window_placements.insert(
        "icons/search.svg".into(),
        WindowPlacement {
            x: -900.0,
            y: 50.0,
            width: 760.0,
            height: 520.0,
            maximized: false,
        },
    );
    let json = serde_json::to_string(&config).unwrap();
    let mut restored: AppConfig = serde_json::from_str(&json).unwrap();
    restored.reset_preferences();
    assert_eq!(
        restored.tool_window_placements,
        config.tool_window_placements
    );
    let mut legacy = serde_json::to_value(&config).unwrap();
    legacy
        .as_object_mut()
        .unwrap()
        .remove("tool_window_placements");
    assert!(serde_json::from_value::<AppConfig>(legacy)
        .unwrap()
        .tool_window_placements
        .is_empty());
}
