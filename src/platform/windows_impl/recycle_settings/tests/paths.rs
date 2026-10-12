#[test]
fn forward_slash_root_reads_the_same_windows_settings() {
    let executable = std::env::current_exe().unwrap();
    let root = executable.ancestors().last().unwrap().to_str().unwrap();
    let native = super::read_recycle_bin_config(root.into()).unwrap();
    let forward = super::read_recycle_bin_config(root.replace('\\', "/")).unwrap();

    assert_eq!(forward.max_size_mb, native.max_size_mb);
    assert_eq!(forward.nuke_on_delete, native.nuke_on_delete);
}
