use super::AppConfig;

#[test]
fn viewer_defaults_and_saved_choices_round_trip() {
    let legacy: AppConfig = serde_json::from_str("{}").unwrap();
    assert!(legacy.viewer_markdown_rendered);
    assert!(legacy.viewer_syntax_highlighting);
    assert!(legacy.viewer_media_autoplay);
    let config = AppConfig {
        viewer_markdown_rendered: false,
        viewer_syntax_highlighting: false,
        viewer_media_autoplay: false,
        ..AppConfig::default()
    };
    let saved: AppConfig = serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();
    assert!(!saved.viewer_markdown_rendered);
    assert!(!saved.viewer_syntax_highlighting);
    assert!(!saved.viewer_media_autoplay);
}
