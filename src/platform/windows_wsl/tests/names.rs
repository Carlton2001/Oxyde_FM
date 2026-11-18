use super::node;
use crate::platform::{is_wsl_path, wsl_relative, WSL_LEGACY_ROOT, WSL_ROOT};

#[test]
fn a_distribution_is_a_share_of_the_wsl_root() {
    let ubuntu = node(WSL_ROOT, "Ubuntu".into()).unwrap();
    assert_eq!(ubuntu.path, r"\\wsl.localhost\Ubuntu");
    assert!(ubuntu.has_subdirs);
    assert_eq!(node(WSL_LEGACY_ROOT, "Ubuntu".into()).unwrap().path, r"\\wsl$\Ubuntu");
}

#[test]
fn both_names_of_the_root_are_wsl_paths() {
    for path in [r"\\wsl.localhost\Ubuntu\home", r"\\WSL$\Ubuntu\home", r"\\wsl$"] {
        assert!(is_wsl_path(path), "{path}");
    }
    assert_eq!(wsl_relative(r"\\wsl$\Ubuntu\home"), Some(r"\Ubuntu\home"));
    for path in [r"\\wsl.localhostx\Ubuntu", r"\\wsl$x\Ubuntu", r"\\server\wsl$"] {
        assert!(!is_wsl_path(path), "{path}");
    }
}

#[test]
fn a_name_cannot_leave_the_wsl_root() {
    for name in ["", r"Ubuntu\home", "Ubuntu/home"] {
        assert!(node(WSL_ROOT, name.into()).is_none());
    }
}
