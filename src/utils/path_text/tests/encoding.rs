use super::*;

#[test]
fn valid_paths_keep_their_text() {
    let path = Path::new("/home/user/Été/rapport.txt");
    assert_eq!(encode(path), "/home/user/Été/rapport.txt");
    assert_eq!(decode(&encode(path)), path);
    assert!(!is_encoded(&encode(path)));
}

#[cfg(unix)]
mod unix {
    use super::*;
    use std::{ffi::OsStr, os::unix::ffi::OsStrExt};

    fn raw(bytes: &[u8]) -> PathBuf {
        PathBuf::from(OsStr::from_bytes(bytes))
    }

    #[test]
    fn invalid_bytes_survive_the_round_trip() {
        let path = raw(b"/tmp/caf\xe9/\xff\xfeend\xc3");
        let text = encode(&path);
        assert!(is_encoded(&text));
        assert_eq!(text, "/tmp/caf\0E9/\0FF\0FEend\0C3");
        assert_eq!(decode(&text), path);
    }

    #[test]
    fn a_lookalike_name_keeps_a_distinct_text() {
        let invalid = raw(b"/tmp/a\xffb");
        let replacement = Path::new("/tmp/a\u{FFFD}b");
        assert_ne!(encode(&invalid), encode(replacement));
        assert_eq!(display(&encode(&invalid)), "/tmp/a\u{FFFD}b");
    }

    #[test]
    fn an_encoded_text_cannot_be_opened_without_decoding() {
        let directory = tempfile::tempdir().unwrap();
        let invalid = directory.path().join(OsStr::from_bytes(b"a\xffb"));
        std::fs::write(&invalid, b"kept").unwrap();
        std::fs::write(directory.path().join("a\u{FFFD}b"), b"other").unwrap();
        let text = encode(&invalid);
        assert!(std::fs::read(&text).is_err());
        assert_eq!(std::fs::read(decode(&text)).unwrap(), b"kept");
    }

    #[test]
    fn a_malformed_escape_stays_unopenable() {
        assert!(decode("/tmp/a\0Zb").as_os_str().as_bytes().contains(&0));
        assert!(decode("/tmp/a\0").as_os_str().as_bytes().contains(&0));
    }
}
