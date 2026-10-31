use super::node;

#[test]
fn a_distribution_is_a_share_of_the_wsl_root() {
    let node = node("Ubuntu".into()).unwrap();
    assert_eq!(node.path, r"\\wsl.localhost\Ubuntu");
    assert!(node.has_subdirs);
}

#[test]
fn a_name_cannot_leave_the_wsl_root() {
    for name in ["", r"Ubuntu\home", "Ubuntu/home"] {
        assert!(node(name.into()).is_none());
    }
}
