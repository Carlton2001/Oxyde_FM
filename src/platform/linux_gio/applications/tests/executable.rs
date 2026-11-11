use super::{in_path, unquoted};

#[test]
fn a_quoted_program_loses_its_quotes() {
    assert_eq!(unquoted("\"/home/me/.local/lib/oxyde/current/bin/oxyde\""), "/home/me/.local/lib/oxyde/current/bin/oxyde");
    assert_eq!(unquoted("\"/opt/my \\\"apps\\\"/oxyde\""), "/opt/my \"apps\"/oxyde");
}

#[test]
fn a_plain_program_is_kept() {
    assert_eq!(unquoted("/usr/bin/oxyde"), "/usr/bin/oxyde");
    assert_eq!(unquoted("oxyde"), "oxyde");
}

#[test]
fn a_bare_program_is_found_in_path() {
    let folder = tempfile::tempdir().unwrap();
    let program = folder.path().join("oxyde");
    std::fs::write(&program, b"").unwrap();
    let path = std::env::join_paths(["relative", "/nonexistent", &folder.path().to_string_lossy()]).unwrap();
    assert_eq!(in_path("oxyde", Some(&path)), Some(program));
    assert_eq!(in_path("missing", Some(&path)), None);
    assert_eq!(in_path("bin/oxyde", Some(&path)), None);
    assert_eq!(in_path("/usr/bin/oxyde", None), Some("/usr/bin/oxyde".into()));
}
