use super::{command, handlers, SWITCH};

#[test]
fn the_step_runs_the_quoted_launcher_with_its_switch() {
    assert_eq!(command(r"C:\Programs\Oxyde FM\oxyde-folder-launcher.exe"), format!(r#""C:\Programs\Oxyde FM\oxyde-folder-launcher.exe" {SWITCH}"#));
}

#[test]
fn the_handlers_are_the_sessions() {
    assert_eq!(handlers(1), r"Software\Microsoft\Windows\CurrentVersion\Explorer\SessionInfo\1\DirectLaunchHandlers");
}
