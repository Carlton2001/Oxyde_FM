use super::entered_child;

#[test]
fn only_one_level_down_glides() {
    assert!(entered_child("/home", "/home/alice"));
    assert!(!entered_child("/home", "/home/alice/Documents"));
    assert!(!entered_child("/home/alice", "/home"));
    assert!(!entered_child("/home", "/homeBackup"));
    assert!(!entered_child("", "/"));
}

#[cfg(not(target_os = "windows"))]
#[test]
fn a_backslash_is_part_of_a_linux_name() {
    assert!(entered_child("/x", "/x/a\\b"));
    assert!(!entered_child("/x/a", "/x/a\\b"));
}

#[cfg(target_os = "windows")]
#[test]
fn windows_paths_glide_one_level_down() {
    assert!(entered_child(r"C:\Users", r"C:\Users\Alice"));
    assert!(entered_child(r"C:\", r"C:\Users"));
    assert!(!entered_child(r"C:\Users", r"C:\Users\Alice\Documents"));
    assert!(!entered_child(r"C:\Users\Alice", r"C:\Users"));
    assert!(!entered_child(r"C:\Users", r"C:\UsersBackup"));
    assert!(!entered_child("", r"C:\"));
    assert!(!entered_child(r"C:\Users", r"D:\Users\Alice"));
}
