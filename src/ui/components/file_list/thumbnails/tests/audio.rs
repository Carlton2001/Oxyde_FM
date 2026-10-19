use super::{supports_thumbnail, Source};
use oxyde_lib::models::{file_entry::NativeFileKind, FileEntry};

#[test]
fn grid_admits_audio_by_extension_or_native_kind_and_hover_recognizes_audio() {
    for name in ["song.mp3", "song.FLAC", "song.m4a", "song.ogg"] {
        let file = FileEntry { name: name.into(), ..Default::default() };
        assert!(supports_thumbnail(&file), "{name}");
        assert!(Source::for_file(&file).audio);
        assert_eq!(crate::ui::components::icons::get_preview_type(name), crate::ui::components::icons::PreviewType::Audio);
    }
    let file = FileEntry { name: "without-extension".into(), native_kind: Some(NativeFileKind::Audio), ..Default::default() };
    assert!(supports_thumbnail(&file));
    assert!(Source::for_file(&file).audio);
    assert!(!supports_thumbnail(&FileEntry { name: "notes.txt".into(), ..Default::default() }));
}
