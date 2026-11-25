use super::super::ConflictDialogState;
use super::decision;
use oxyde_lib::models::{ConflictAction, ConflictEntry, FileEntry};

fn conflict(path: &str, is_dir: bool) -> ConflictEntry {
    ConflictEntry {
        name: path.rsplit('/').next().unwrap().into(),
        source: FileEntry { path: path.into(), is_dir, modified: 1, ..Default::default() },
        target: FileEntry { modified: 2, ..Default::default() },
    }
}

#[test]
fn choices_in_one_pass_match_choices_one_by_one() {
    let conflicts = vec![
        conflict("a", true),
        conflict("a/one", false),
        conflict("a/b", true),
        conflict("a/b/two", false),
        conflict("c", true),
        conflict("c/three", false),
        conflict("d", false),
    ];
    for indices in [vec![0, 4], vec![2, 0, 6], vec![4, 1], vec![6, 2]] {
        for action in [ConflictAction::Skip, ConflictAction::Replace] {
            let start = || {
                let mut state = ConflictDialogState { advanced: true, ..Default::default() };
                // A first answer sets the defaults the later ones override.
                state.record_choice(&conflicts, 3, ConflictAction::Skip);
                state
            };
            let mut one_by_one = start();
            for &index in &indices {
                one_by_one.record_choice(&conflicts, index, action);
            }
            let mut bulk = start();
            bulk.record_choices(&conflicts, &indices, action);
            for c in &conflicts {
                assert_eq!(decision(&bulk, &c.source.path), decision(&one_by_one, &c.source.path), "{} {indices:?}", c.source.path);
            }
            assert_eq!(bulk.automatic, one_by_one.automatic);
        }
    }
}
