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
