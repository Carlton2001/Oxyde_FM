use super::*;

#[test]
fn a_marker_quoted_before_the_report_does_not_hide_it() {
    let report = ArchiveRecoveryFailure {
        completed: false,
        locations: vec!["/kept/archive.zip".into()],
        omitted: 0,
        cause: format!("cannot read {}x", ArchiveRecoveryFailure::MARKER),
    };
    let message = format!("Cannot rebuild {}name.zip: {report}", ArchiveRecoveryFailure::MARKER);
    assert_eq!(ArchiveRecoveryFailure::from_message(&message), Some(report));
}
