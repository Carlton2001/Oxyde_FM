use super::*;
use crate::platform::ShellDeleteOutcome;

#[test]
fn diagnostic_size_defaults_to_256_and_only_allows_the_two_comparison_sizes() {
    assert_eq!(parse_batch_size(None), Some(256));
    for (value, expected) in [
        ("64", Some(64)),
        ("256", Some(256)),
        ("0", None),
        ("-1", None),
        ("17140", None),
        ("", None),
        (" 256", None),
        ("256x", None),
    ] {
        assert_eq!(
            parse_batch_size(Some(std::ffi::OsStr::new(value))),
            expected
        );
    }
}

fn selection(root: &std::path::Path) -> Vec<PreparedDelete> {
    (0..3)
        .map(|index| {
            let path = root.join(format!("fixture-{index}"));
            std::fs::write(&path, b"fixture").unwrap();
            PreparedDelete::new(&path).unwrap()
        })
        .collect()
}

#[test]
fn every_confirmed_success_is_journaled_despite_an_earlier_batch_error() {
    let root = tempfile::tempdir().unwrap();
    let selected = selection(root.path());
    let outcomes = vec![
        ShellDeleteOutcome::from_result(Err("first failed".into())),
        ShellDeleteOutcome::from_result(Ok(())),
        ShellDeleteOutcome {
            destroyed: false,
            completed: true,
            error: Some("later cancellation".into()),
            effects: Vec::new(),
        },
    ];
    let mut journal = Vec::new();
    let failures = record_batch_receipts(&selected, outcomes, &mut |entry| journal.push(entry));
    // Each refused root is reported on its own instead of hiding the others.
    let failed: Vec<_> = failures.iter().map(|(path, _)| path.clone()).collect();
    assert_eq!(failed, vec![selected[0].path().to_path_buf(), selected[2].path().to_path_buf()]);
    assert_eq!(journal.len(), 2);
    assert_eq!(journal[0].path, selected[1].path().to_string_lossy());
    assert_eq!(journal[1].path, selected[2].path().to_string_lossy());
    assert!(selected.iter().all(|item| item.path().exists())); // Synthetic receipts only.
}

#[test]
fn missing_receipts_do_not_discard_the_confirmed_prefix() {
    let root = tempfile::tempdir().unwrap();
    let selected = selection(root.path());
    let mut journal = Vec::new();
    let failures = record_batch_receipts(
        &selected,
        vec![ShellDeleteOutcome::from_result(Ok(()))],
        &mut |entry| journal.push(entry),
    );
    assert_eq!(failures.len(), 2);
    assert!(failures.iter().all(|(_, error)| error == "Missing batch deletion receipts"));
    assert_eq!(journal.len(), 1);
}

#[test]
fn directories_stay_outside_the_flat_batch() {
    let root = tempfile::tempdir().unwrap();
    let directory = PreparedDelete::new(root.path()).unwrap();
    assert!(directory.batch_file().is_none());
    let selected = selection(root.path());
    assert!(selected[0].batch_file().is_some());
}

#[test]
fn an_item_destroyed_instead_of_recycled_is_reported_and_never_journaled() {
    let root = tempfile::tempdir().unwrap();
    let selected = selection(root.path());
    let outcomes = vec![
        ShellDeleteOutcome::from_result(Ok(())),
        ShellDeleteOutcome {
            completed: true,
            error: Some("Windows deleted the item permanently instead of recycling it (0x80270036)".into()),
            effects: Vec::new(),
            destroyed: true,
        },
        ShellDeleteOutcome::from_result(Ok(())),
    ];
    let mut journal = Vec::new();
    let failures = record_batch_receipts(&selected, outcomes, &mut |entry| journal.push(entry));
    // Undo restores only what reached the Recycle Bin.
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].0, selected[1].path());
    let journaled: Vec<_> = journal.iter().map(|entry| entry.path.clone()).collect();
    assert_eq!(journaled, [selected[0].path().to_string_lossy(), selected[2].path().to_string_lossy()]);
}
