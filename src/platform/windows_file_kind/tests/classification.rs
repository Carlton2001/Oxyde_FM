use super::*;

#[test]
fn shell_classification_uses_native_known_extensions_and_handles_unknowns() {
    let platform = crate::platform::platform();
    for (extension, expected) in [
        (".txt", NativeFileKind::Documents),
        (".png", NativeFileKind::Images),
        (".zip", NativeFileKind::Archives),
    ] {
        assert_eq!(
            platform.file_kind_for_extension(extension),
            Some(expected),
            "{extension}"
        );
    }
    assert_eq!(
        platform.file_kind_for_extension(".oxyde_unknown_73ae51"),
        None
    );
    for extension in ["", ".", "png", ".png\0.exe", ".path/file", ".path\\file"] {
        assert_eq!(platform.file_kind_for_extension(extension), None);
    }
}
