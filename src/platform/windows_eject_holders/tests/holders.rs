use super::*;

#[test]
fn keeps_the_executable_of_nt_paths() {
    assert_eq!(executable(r"\Device\HarddiskVolume3\Windows\explorer.exe"), "explorer.exe");
    assert_eq!(executable("System"), "System");
}

#[test]
fn names_are_sorted_and_unique() {
    let processes = vec![r"\Device\V\b.exe".to_string(), r"\Device\V\a.exe".to_string(), r"\Device\W\b.exe".to_string()];
    assert_eq!(names(processes), ["a.exe", "b.exe"]);
}

#[test]
fn kernel_process_only_stays_when_alone() {
    assert_eq!(names(vec!["System".into(), r"\Device\V\explorer.exe".into()]), ["explorer.exe"]);
    assert_eq!(names(vec!["System".into()]), ["System"]);
}

#[test]
fn system_log_query_accepts_the_filter() {
    assert_eq!(vetoes("NO\\SUCH\\DEVICE", Duration::from_secs(60)).unwrap(), Vec::<String>::new());
}
