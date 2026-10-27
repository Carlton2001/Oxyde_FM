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
