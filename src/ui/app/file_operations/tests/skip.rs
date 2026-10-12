use super::actionable_transfer_paths;
use oxyde_lib::models::ConflictAction::{Replace, Skip};
use std::collections::HashMap;

#[test]
fn ignoring_every_source_does_not_queue_an_empty_successful_transfer() {
    let choices = HashMap::from([("a".into(), Skip), ("b".into(), Skip)]);
    assert!(actionable_transfer_paths(vec!["a".into(), "b".into()], &choices).is_empty());
}

#[test]
fn ignoring_conflicts_still_transfers_non_conflicting_sources() {
    let choices = HashMap::from([("a".into(), Skip), ("b".into(), Replace)]);
    assert_eq!(
        actionable_transfer_paths(vec!["a".into(), "b".into(), "c".into()], &choices),
        vec!["b", "c"]
    );
}

#[test]
fn skipped_child_does_not_discard_an_entire_directory() {
    let choices = HashMap::from([("directory/child".into(), Skip)]);
    assert_eq!(
        actionable_transfer_paths(vec!["directory".into()], &choices),
        vec!["directory"]
    );
}
