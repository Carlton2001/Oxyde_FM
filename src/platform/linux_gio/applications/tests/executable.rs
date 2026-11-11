use super::unquoted;

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
