use super::*;

#[test]
fn an_open_file_names_its_process() {
    let directory = tempfile::tempdir().unwrap();
    let point = directory.path().to_string_lossy().into_owned();
    assert!(!holders(std::slice::from_ref(&point)).contains(&own_name()));
    let _file = std::fs::File::create(directory.path().join("open")).unwrap();
    assert!(holders(&[point]).contains(&own_name()));
}

fn own_name() -> String {
    name(Path::new("/proc/self")).unwrap()
}

#[test]
fn a_mapped_path_keeps_its_spaces() {
    let line = "7f1c2a000000-7f1c2a021000 r--p 00000000 08:11 1234                       /run/media/user/Ma Clé/lib.so";
    assert_eq!(mapped_path(line), Some("/run/media/user/Ma Clé/lib.so"));
    assert_eq!(mapped_path("7ffd5e9f6000-7ffd5ea17000 rw-p 00000000 00:00 0 "), None);
    assert_eq!(mapped_path("7ffd5e9f6000-7ffd5ea17000 rw-p 00000000 00:00 0                          [stack]"), Some("[stack]"));
}
