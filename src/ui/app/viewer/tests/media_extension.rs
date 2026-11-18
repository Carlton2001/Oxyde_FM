use super::file_extension;

#[test]
fn a_name_without_an_extension_is_not_taken_for_one() {
    assert_eq!(file_extension("song.MP3"), "mp3");
    assert_eq!(file_extension("mp3"), "");
    assert_eq!(file_extension(".mp3"), "");
}
