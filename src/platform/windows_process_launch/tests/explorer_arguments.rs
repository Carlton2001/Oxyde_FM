use super::*;

fn argument(path: &str) -> String {
    let mut command = Command::new("explorer.exe");
    explorer_path_argument(&mut command, path).unwrap();
    let arguments: Vec<_> = command.get_args().map(|arg| arg.to_string_lossy().into_owned()).collect();
    assert_eq!(arguments.len(), 1);
    arguments[0].clone()
}

// Explorer reads `D:\Factures\2024,03.pdf` unquoted as two arguments.
#[test]
fn paths_explorer_would_split_are_quoted() {
    assert_eq!(argument(r"D:\Factures\2024,03.pdf"), r#""D:\Factures\2024,03.pdf""#);
    assert_eq!(argument(r"D:\a;b"), r#""D:\a;b""#);
    assert_eq!(argument(r"D:\x=y\"), r#""D:\x=y""#);
    assert_eq!(argument(r"D:\My Folder\"), r#""D:\My Folder""#);
    assert_eq!(argument(r"C:\Windows"), r"C:\Windows");
    assert_eq!(argument(r"C:\"), r"C:\");
    assert!(explorer_path_argument(&mut Command::new("explorer.exe"), "D:\\a\"b").is_err());
}

#[test]
fn a_lone_surrogate_name_reaches_explorer_and_a_leftover_nul_is_refused() {
    use std::os::windows::ffi::OsStrExt;
    let mut command = Command::new("explorer.exe");
    explorer_path_argument(&mut command, "D:\\a\0D800b").unwrap();
    let argument: Vec<u16> = command.get_args().next().unwrap().encode_wide().collect();
    let mut expected: Vec<u16> = "D:\\a".encode_utf16().collect();
    expected.extend([0xD800, u16::from(b'b')]);
    assert_eq!(argument, expected);
    assert!(explorer_path_argument(&mut Command::new("explorer.exe"), "D:\\a\0Zb").is_err());
}
