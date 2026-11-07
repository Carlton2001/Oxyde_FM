use super::*;

#[test]
fn service_file_starts_the_entry_program_quoted() {
    assert_eq!(service_file(Path::new("/home/a/.local/lib/oxyde/current/bin/oxyde")),
        "[D-BUS Service]\nName=org.freedesktop.FileManager1\nExec=\"/home/a/.local/lib/oxyde/current/bin/oxyde\"\n");
    assert_eq!(service_file(Path::new("/home/my \"x\" $HOME\\/bin/oxyde")),
        "[D-BUS Service]\nName=org.freedesktop.FileManager1\nExec=\"/home/my \\\"x\\\" \\$HOME\\\\/bin/oxyde\"\n");
}

#[test]
fn the_service_file_is_named_after_oxyde() {
    assert_eq!(SERVICE_FILE, "io.github.Carlton2001.Oxyde.FileManager1.service");
}
