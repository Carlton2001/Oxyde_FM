use super::{location, locations, named};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

fn current() -> &'static Path { Path::new(r"C:\Work") }

#[test]
fn a_drive_root_survives_the_quote_its_backslash_escaped() {
    assert_eq!(named("C:\"", current()), Some(PathBuf::from(r"C:\")));
    assert_eq!(named(r"D:\", current()), Some(PathBuf::from(r"D:\")));
    assert_eq!(named("D:", current()), Some(PathBuf::from(r"D:\")));
}

#[test]
fn a_path_loses_its_trailing_separator_and_its_dots() {
    assert_eq!(named(r"C:\Users\", current()), Some(PathBuf::from(r"C:\Users")));
    assert_eq!(named(r"C:\Users\Public\..\.\Default", current()), Some(PathBuf::from(r"C:\Users\Default")));
    assert_eq!(named(r"\\server\share\folder\", current()), Some(PathBuf::from(r"\\server\share\folder")));
}

#[test]
fn a_relative_path_starts_from_the_current_folder() {
    assert_eq!(named(r"sub\folder", current()), Some(PathBuf::from(r"C:\Work\sub\folder")));
}

#[test]
fn switches_and_shell_names_are_no_folder() {
    for argument in ["--restore-folder-handler", "-x", r"/select,C:\file.txt", "::{20D04FE0-3AEA-1069-A2D8-08002B30309D}", "shell:Downloads", "shell:::{645FF040-5081-101B-9F08-00AA002F954E}", "file:///C:/Users", "ms-settings:network", "", "\""] {
        assert_eq!(named(argument, current()), None, "{argument}");
    }
}

#[test]
fn a_file_stands_for_its_folder_and_a_missing_location_is_left_out() {
    let folder = tempfile::tempdir().unwrap();
    let root = std::path::absolute(folder.path()).unwrap();
    let file = root.join("note.txt");
    std::fs::write(&file, b"").unwrap();
    let expected = root.to_str().unwrap().to_owned();
    assert_eq!(location(file.to_str().unwrap(), current()), Some(expected.clone()));
    assert_eq!(location(&format!("{expected}\\"), current()), Some(expected.clone()));
    let arguments = [file.clone().into_os_string(), root.join("missing").into_os_string(), OsString::from("--flag")];
    assert_eq!(locations(arguments.into_iter(), current()), [expected]);
}

#[test]
fn items_to_select_are_existing_paths_with_a_folder() {
    let folder = tempfile::tempdir().unwrap();
    let file = folder.path().join("report.txt");
    std::fs::write(&file, b"").unwrap();
    let file = file.to_str().unwrap().to_owned();
    let missing = folder.path().join("missing.txt").to_str().unwrap().to_owned();
    assert_eq!(super::items(&[file.clone(), missing, "report.txt".to_owned(), r"C:\".to_owned()]), [file]);
}
