use super::archive_name_for_format;
#[test]
fn format_switch_preserves_name_and_replaces_complete_suffix() {
    assert_eq!(archive_name_for_format("report.v2.tar.zst", "tar.zst", "zip"), "report.v2.zip");
    assert_eq!(archive_name_for_format("report.ZIP", "zip", "7z"), "report.7z");
    assert_eq!(archive_name_for_format("renamed", "zip", "tar.zst"), "renamed.tar.zst");
    assert_eq!(archive_name_for_format("report.txt.zip", "zip", "tar"), "report.txt.tar");
}

#[test]
fn compressed_tar_identifiers_create_their_compound_suffix() {
    use super::archive_extension;
    assert_eq!(archive_extension("zip"), "zip");
    assert_eq!(archive_extension("7z"), "7z");
    assert_eq!(archive_extension("tar"), "tar");
    for format in ["zst", "gz", "xz", "bz2", "lz4"] {
        assert_eq!(archive_extension(format), format!("tar.{format}"));
    }
    assert_eq!(archive_name_for_format("report.tar.zst", "tar.zst", "tar.xz"), "report.tar.xz");
}
