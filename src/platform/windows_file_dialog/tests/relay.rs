use super::super::request;
use super::{answer, collect, shown, PIPES};
use crate::platform::{FileChooserAnswer, FileChooserParent, FileChooserPurpose};
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[test]
fn the_answer_follows_the_word_that_the_window_is_shown() {
    assert_eq!(shown(b"sho"), None);
    assert_eq!(shown(b"shown\n"), Some(&b""[..]));
    assert_eq!(answer(b"shown\nok\npath\tC:\\a.txt\n"), Some("ok\npath\tC:\\a.txt\n"));
    assert_eq!(answer(b"shown\ncancel\n"), Some("cancel\n"));
}

#[test]
fn a_window_closed_without_an_answer_gives_none() {
    assert_eq!(answer(b"shown\n"), None);
    assert_eq!(answer(b"ok\npath\tC:\\a.txt\n"), None);
    assert_eq!(answer(b"shown\nok"), None);
}

fn dialog(asked: &[&str]) -> (crate::platform::windows_instance::Pipe, crate::platform::FileChooserRequest) {
    let name = format!("{PIPES}test.{}.{}", std::process::id(), uuid::Uuid::new_v4().simple());
    let pipe = crate::platform::windows_instance::create(&name).unwrap();
    let mut sent = vec![name];
    sent.extend(asked.iter().map(|line| (*line).to_owned()));
    (pipe, request(&sent, None).unwrap())
}

#[test]
fn a_dialog_is_answered_on_its_pipe_once_its_window_is_shown() {
    let (pipe, mut request) = dialog(&["purpose\tsave", "name\treport", "extension\ttxt", "owner\t77", "filter\tText\t*.txt", "filter\tAll\t*.*"]);
    assert_eq!(request.purpose, FileChooserPurpose::Save { name: "report".into() });
    assert_eq!(request.default_extension.as_deref(), Some("txt"));
    assert_eq!(request.parent, Some(FileChooserParent::Windows(77)));
    assert_eq!(request.current_filter, Some(0));
    assert!(matches!(request.withdrawn.try_recv(), Ok(None)));
    request.shown.take().unwrap().send(()).unwrap();
    let chosen = FileChooserAnswer { paths: vec![PathBuf::from(r"C:\Users\report.txt")], filter: Some(0), choices: Vec::new() };
    request.answer.send(Some(chosen)).unwrap();
    let answer = collect(pipe, Instant::now() + Duration::from_secs(10));
    assert_eq!(answer.as_deref(), Some("ok\nfilter\t1\npath\tC:\\Users\\report.txt\n"));
}

#[test]
fn a_cancelled_dialog_says_so_and_folders_have_no_filters() {
    let (pipe, mut request) = dialog(&["purpose\tfolder", "multiple\t1", "filter\tText\t*.txt"]);
    assert!(request.directory && request.multiple && request.filters.is_empty() && request.current_filter.is_none());
    request.shown.take().unwrap().send(()).unwrap();
    request.answer.send(None).unwrap();
    assert_eq!(collect(pipe, Instant::now() + Duration::from_secs(10)).as_deref(), Some("cancel\n"));
}

#[test]
fn a_window_that_never_opens_or_closes_unanswered_leaves_the_system_dialog() {
    let (pipe, request) = dialog(&["purpose\topen"]);
    drop(request);
    assert_eq!(collect(pipe, Instant::now() + Duration::from_millis(300)), None);
    let (pipe, mut request) = dialog(&["purpose\topen"]);
    request.shown.take().unwrap().send(()).unwrap();
    drop(request);
    assert_eq!(collect(pipe, Instant::now() + Duration::from_secs(10)), None);
}
