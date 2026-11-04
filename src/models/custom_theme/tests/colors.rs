use super::*;

#[test]
fn hex_colors_accept_common_spellings() {
    for value in ["#3d5af1", "3D5AF1", "  #3d5aF1 "] {
        assert_eq!(parse_hex_color(value), Some(0x3d5af1), "{value}");
    }
    assert_eq!(parse_hex_color("#fa0"), Some(0xffaa00));
    for value in ["", "#", "#12345", "#1234567", "#gg0000", "+12345", "rgb(1,2,3)"] {
        assert_eq!(parse_hex_color(value), None, "{value}");
    }
    assert_eq!(format_hex_color(0x0a0b0c), "#0a0b0c");
}

#[test]
fn colors_round_trip_and_invalid_fields_keep_the_base() {
    let colors = [0x111111, 0x222222, 0x333333, 0x444444, 0xeeeeee, 0x3d5af1];
    let mut theme = CustomTheme::new("theme-1".into(), "Mine".into(), ThemeId::OneDark, colors);
    assert_eq!(theme.colors([0; 6]), colors);
    assert_eq!(theme.base(), ThemeId::OneDark);
    theme.accent = "blue".into();
    assert_eq!(theme.colors([9; 6])[5], 9);
}

#[test]
fn selection_round_trips_and_missing_themes_fall_back() {
    let custom = vec![CustomTheme::new("theme-2".into(), "Mine".into(), ThemeId::Light, [0; 6])];
    for choice in [ThemeChoice::Builtin(ThemeId::Monokai), ThemeChoice::Custom("theme-2".into())] {
        assert_eq!(ThemeChoice::from_config(&choice.to_config(), &custom), choice);
    }
    assert_eq!(ThemeChoice::from_config("custom:theme-9", &custom), ThemeChoice::default());
    // Older versions read a user theme as the default built-in theme.
    assert_eq!(ThemeId::from_id("custom:theme-2"), ThemeId::default());
    assert_eq!(CustomTheme::next_id(&custom), "theme-3");
    assert_eq!(CustomTheme::next_id(&[]), "theme-1");
}
