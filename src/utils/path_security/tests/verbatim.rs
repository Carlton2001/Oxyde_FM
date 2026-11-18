use super::*;

#[test]
fn verbatim_paths_keep_trailing_dots_and_spaces() {
    let verbatim = r"\\?\C:\d\foo.";
    assert_eq!(validate_path(verbatim).unwrap(), PathBuf::from(verbatim));
    assert_eq!(validate_path(r"C:\d\foo.").unwrap(), PathBuf::from(r"C:\d\foo"));
}
