use super::*;

#[test]
fn extended_length_prefixes_give_way_to_the_usual_forms() {
    assert_eq!(without_verbatim_prefix(r"\\?\C:\Users\a.pdf"), r"C:\Users\a.pdf");
    assert_eq!(without_verbatim_prefix(r"\\?\UNC\srv\share\a.pdf"), r"\\srv\share\a.pdf");
    assert_eq!(without_verbatim_prefix(r"\\?\UNC\wsl.localhost\Ubuntu\home"), r"\\wsl.localhost\Ubuntu\home");
    // A volume mounted without a drive letter has no other form.
    let volume = r"\\?\Volume{00000000-0000-0000-0000-000000000001}\photos";
    assert_eq!(without_verbatim_prefix(volume), volume);
    assert_eq!(without_verbatim_prefix(r"D:\plain"), r"D:\plain");
    assert_eq!(without_verbatim_prefix("/home/user"), "/home/user");
}
