use super::{answer_text, asked, patterns, request, Asked};
use crate::platform::{FileChooserAnswer, FileTypeFilter, FileTypePattern};
use std::path::PathBuf;

fn lines(text: &[&str]) -> Vec<String> {
    text.iter().map(|line| (*line).to_owned()).collect()
}

fn globs(patterns: &[&str]) -> Vec<FileTypePattern> {
    patterns.iter().map(|pattern| FileTypePattern::Glob((*pattern).to_owned())).collect()
}

#[test]
fn a_filter_lists_its_patterns_and_every_file_needs_no_extension() {
    assert_eq!(patterns("*.png; *.jpg;"), globs(&["*.png", "*.jpg"]));
    assert_eq!(patterns("*.*"), globs(&["*"]));
    assert_eq!(patterns(""), Vec::new());
}

#[test]
fn what_is_asked_is_read_line_by_line() {
    let read = asked(&lines(&[
        "purpose\topen", "multiple\t1", "title\tOpen a file", "accept\t&Open", "folder\tC:\\Users", "owner\t1050024",
        "filter\tImages\t*.png;*.jpg", "filter\tAll files\t*.*", "filter-index\t2", "later\tignored", "no tab",
    ]));
    assert_eq!(read, Asked {
        purpose: "open".into(),
        multiple: true,
        title: "Open a file".into(),
        accept: Some("Open".into()),
        folder: Some(PathBuf::from(r"C:\Users")),
        owner: Some(1050024),
        filters: vec![
            FileTypeFilter { name: "Images".into(), patterns: globs(&["*.png", "*.jpg"]) },
            FileTypeFilter { name: "All files".into(), patterns: globs(&["*"]) },
        ],
        filter: Some(2),
        ..Default::default()
    });
    let read = asked(&lines(&["purpose\tsave", "name\treport", "extension\t.txt", "folder\tDocuments", "owner\t0"]));
    assert_eq!(read, Asked { purpose: "save".into(), name: "report".into(), extension: Some("txt".into()), ..Default::default() });
}

#[test]
fn an_answer_counts_filters_as_windows_does() {
    assert_eq!(answer_text(None), "cancel\n");
    let answer = FileChooserAnswer { paths: vec![PathBuf::from(r"C:\a.txt"), PathBuf::from("C:\\b\n.txt")], filter: Some(1), choices: Vec::new() };
    assert_eq!(answer_text(Some(&answer)), "ok\nfilter\t2\npath\tC:\\a.txt\n");
}

#[test]
fn a_dialog_without_a_pipe_of_its_own_is_not_one() {
    assert!(request(&lines(&[r"\\.\pipe\Other", "purpose\topen"]), None).is_none());
    assert!(request(&[], None).is_none());
}
