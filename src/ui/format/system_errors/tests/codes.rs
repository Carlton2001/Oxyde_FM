use super::*;

#[test]
fn code_is_read_from_the_end_of_the_message() {
    assert_eq!(code("Cannot copy \"C:\\a\": Accès refusé. (os error 5)"), Some(5));
    assert_eq!(code("(os error 2) then (os error 32)"), Some(32));
    assert_eq!(code("no code here"), None);
    assert_eq!(code("(os error x)"), None);
}

#[cfg(target_os = "windows")]
#[test]
fn common_windows_errors_are_named() {
    assert_eq!(message("x (os error 5)"), Some(Message::SystemErrorAccessDenied));
    assert_eq!(message("x (os error 32)"), Some(Message::SystemErrorInUse));
    assert_eq!(message("x (os error 112)"), Some(Message::SystemErrorDiskFull));
    assert_eq!(message("x (os error 80)"), Some(Message::SystemErrorAlreadyExists));
    assert_eq!(message("x (os error 21)"), Some(Message::SystemErrorNotReady));
    assert_eq!(message("x (os error 53)"), Some(Message::SystemErrorNetwork));
    assert_eq!(message("x (os error 4390)"), None);
}

#[cfg(not(target_os = "windows"))]
#[test]
fn common_unix_errors_are_named() {
    assert_eq!(message("x (os error 13)"), Some(Message::SystemErrorAccessDenied));
    assert_eq!(message("x (os error 28)"), Some(Message::SystemErrorDiskFull));
    assert_eq!(message("x (os error 17)"), Some(Message::SystemErrorAlreadyExists));
}
