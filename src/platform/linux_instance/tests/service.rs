use super::*;

#[test]
fn service_file_starts_the_entry_program_quoted() {
    assert_eq!(service_file(FILE_MANAGER, Path::new("/home/a/.local/lib/oxyde/current/bin/oxyde"), None),
        "[D-BUS Service]\nName=org.freedesktop.FileManager1\nExec=\"/home/a/.local/lib/oxyde/current/bin/oxyde\"\n");
    assert_eq!(service_file(FILE_MANAGER, Path::new("/home/my \"x\" $HOME\\/bin/oxyde"), None),
        "[D-BUS Service]\nName=org.freedesktop.FileManager1\nExec=\"/home/my \\\"x\\\" \\$HOME\\\\/bin/oxyde\"\n");
}

#[test]
fn service_file_passes_its_argument_after_the_program() {
    assert_eq!(service_file("org.freedesktop.impl.portal.desktop.oxyde", Path::new("/home/a/.local/bin/oxyde"), Some("--portal")),
        "[D-BUS Service]\nName=org.freedesktop.impl.portal.desktop.oxyde\nExec=\"/home/a/.local/bin/oxyde\" --portal\n");
}

#[test]
fn the_service_file_is_named_after_oxyde() {
    assert_eq!(SERVICE_FILE, "io.github.Carlton2001.Oxyde.FileManager1.service");
}
