use super::entered_child;

#[test]
fn only_one_level_down_glides() {
    assert!(entered_child(r"C:\Users", r"C:\Users\Alice"));
    assert!(entered_child(r"C:\", r"C:\Users"));
    assert!(entered_child("/home", "/home/alice"));
    assert!(!entered_child(r"C:\Users", r"C:\Users\Alice\Documents"));
    assert!(!entered_child(r"C:\Users\Alice", r"C:\Users"));
    assert!(!entered_child(r"C:\Users", r"C:\UsersBackup"));
    assert!(!entered_child("", r"C:\"));
    assert!(!entered_child(r"C:\Users", r"D:\Users\Alice"));
}
