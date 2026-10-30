use super::*;

fn names(desktop: &str) -> Vec<&'static str> {
    network_settings(desktop).into_iter().map(|(program, _)| program).collect()
}

#[test]
fn each_desktop_opens_its_own_settings_first() {
    assert_eq!(names("GNOME"), ["gnome-control-center", "nm-connection-editor"]);
    assert_eq!(names("ubuntu:GNOME"), ["gnome-control-center", "nm-connection-editor"]);
    assert_eq!(names("KDE"), ["systemsettings", "systemsettings5", "nm-connection-editor"]);
    assert_eq!(names("X-Cinnamon"), ["cinnamon-settings", "nm-connection-editor"]);
}

#[test]
fn other_desktops_fall_back_on_the_connection_editor() {
    assert_eq!(names("XFCE"), ["nm-connection-editor"]);
    assert_eq!(names(""), ["nm-connection-editor"]);
}

#[test]
fn names_are_matched_whole() {
    assert_eq!(names("NOTGNOME"), ["nm-connection-editor"]);
}
