use super::archive_name_for_format;
#[test]
fn format_switch_preserves_name_and_replaces_complete_suffix() {
    assert_eq!(archive_name_for_format("report.v2.tar.zst", "tar.zst", "zip"), "report.v2.zip");
    assert_eq!(archive_name_for_format("report.ZIP", "zip", "7z"), "report.7z");
    assert_eq!(archive_name_for_format("renamed", "zip", "tar.zst"), "renamed.tar.zst");
    assert_eq!(archive_name_for_format("report.txt.zip", "zip", "tar"), "report.txt.tar");
}
