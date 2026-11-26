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

#[cfg(windows)]
mod windows {
    use super::*;
    use std::{ffi::OsString, os::windows::ffi::OsStringExt};

    fn lone(folder: &Path) -> PathBuf {
        let mut name: Vec<u16> = "a".encode_utf16().collect();
        name.push(0xD800);
        name.extend("b".encode_utf16());
        folder.join(OsString::from_wide(&name))
    }

    #[test]
    fn a_lone_surrogate_name_reaches_its_own_file_only_once_decoded() {
        let directory = tempfile::tempdir().unwrap();
        let invalid = lone(directory.path());
        std::fs::write(&invalid, b"kept").unwrap();
        std::fs::write(directory.path().join("a\u{FFFD}b"), b"other").unwrap();
        let text = encode(&invalid);
        assert!(is_encoded(&text));
        assert!(text.ends_with("a\0D800b"));
        assert!(std::fs::read(&text).is_err());
        assert_eq!(decode(&text), invalid);
        assert_eq!(std::fs::read(decode(&text)).unwrap(), b"kept");
        assert_eq!(display(&text), directory.path().join("a\u{FFFD}b").to_string_lossy());
    }

    #[test]
    fn wide_text_decodes_and_refuses_a_leftover_nul() {
        let directory = tempfile::tempdir().unwrap();
        let invalid = lone(directory.path());
        let expected: Vec<u16> = {
            use std::os::windows::ffi::OsStrExt;
            invalid.as_os_str().encode_wide().chain([0]).collect()
        };
        assert_eq!(wide(encode(&invalid)).unwrap(), expected);
        assert_eq!(wide(&invalid).unwrap(), expected);
        assert_eq!(&hstring(encode(&invalid)).unwrap()[..], &expected[..expected.len() - 1]);
        assert!(wide("C:\\a\0Zb").is_err());
        assert!(hstring("C:\\a\0").is_err());
    }
}

/// The Windows escaping, checked on its UTF-16 so that it runs everywhere.
mod wide {
    use super::super::{decode_wide, encode_wide};

    fn units(text: &str) -> Vec<u16> {
        text.encode_utf16().collect()
    }

    #[test]
    fn lone_surrogates_survive_the_round_trip() {
        let mut name = units(r"C:\dossier\a");
        name.push(0xD800);
        name.extend(units("b"));
        name.push(0xDC01);
        let text = encode_wide(name.iter().copied());
        assert_eq!(text, "C:\\dossier\\a\0D800b\0DC01");
        assert_eq!(decode_wide(&text), name);
    }

    #[test]
    fn valid_names_and_surrogate_pairs_keep_their_text() {
        let name = units(r"C:\Été\😀.txt");
        assert_eq!(encode_wide(name.iter().copied()), r"C:\Été\😀.txt");
        assert_eq!(decode_wide(r"C:\Été\😀.txt"), name);
    }

    #[test]
    fn a_lookalike_name_keeps_a_distinct_text() {
        let lone = encode_wide([0x61, 0xD800]);
        let replacement = encode_wide(units("a\u{FFFD}"));
        assert_ne!(lone, replacement);
    }

    #[test]
    fn a_malformed_escape_keeps_its_nul() {
        for text in ["a\0", "a\0D80", "a\0+D80", "a\0ZZZZ", "a\00041", "a\0D8é0"] {
            assert!(decode_wide(text).contains(&0), "{text:?}");
        }
    }
}
