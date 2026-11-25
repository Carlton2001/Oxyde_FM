use super::*;

#[test]
fn worker_errors_lose_control_and_bidirectional_characters() {
    assert_eq!(displayable("bad\u{202E}txt.exe\n\u{1b}[31m"), "badtxt.exe[31m");
    assert_eq!(displayable("Mot de passe requis : « archive.7z »"), "Mot de passe requis : « archive.7z »");
}
