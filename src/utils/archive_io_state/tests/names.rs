use super::*;

fn member(id: u64, name: &str) -> Frame {
    Frame::new(Message::Entry {
        entry: Entry {
            id,
            name: name.into(),
            directory: false,
            size: 0,
            modified: None,
            created: None,
            accessed: None,
            mode: 0o644,
        },
    })
}

#[test]
fn names_differing_only_by_case_are_one_file_only_on_windows() {
    let limits = Limits { entries: 4, bytes: 0, depth: 4, seconds: 1 };
    let mut state = ExtractionState::new(limits, None, false).unwrap();
    state.accept(&member(0, "src/Makefile")).unwrap();
    assert_eq!(state.accept(&member(1, "src/makefile")).is_err(), cfg!(windows));
    assert_eq!(name_key(std::path::Path::new("A/B")) == name_key(std::path::Path::new("a/b")), cfg!(windows));
}
