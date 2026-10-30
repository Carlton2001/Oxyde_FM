use super::*;

fn names(desktop: &str) -> Vec<&'static str> {
    terminals(desktop).into_iter().map(|(program, _)| program).collect()
}

#[test]
fn the_standard_comes_first_then_the_terminal_of_the_desktop() {
    assert_eq!(names("GNOME")[..2], ["xdg-terminal-exec", "ptyxis"]);
    assert_eq!(names("KDE")[..2], ["xdg-terminal-exec", "konsole"]);
    assert_eq!(names("XFCE")[..2], ["xdg-terminal-exec", "xfce4-terminal"]);
}

#[test]
fn each_terminal_is_tried_once() {
    let mut list = names("KDE");
    let count = list.len();
    list.sort();
    list.dedup();
    assert_eq!(list.len(), count);
}

#[test]
fn a_missing_folder_is_not_mistaken_for_a_missing_terminal() {
    let error = open(Path::new("/nonexistent/oxyde-terminal")).unwrap_err();
    assert!(error.contains("oxyde-terminal"), "{error}");
}
