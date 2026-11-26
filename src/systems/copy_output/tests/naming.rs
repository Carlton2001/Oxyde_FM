use super::temporary_name;
use std::ffi::OsStr;

fn units(name: &std::ffi::OsStr) -> usize {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::ffi::OsStrExt;
        name.encode_wide().count()
    }
    #[cfg(not(target_os = "windows"))]
    {
        use std::os::unix::ffi::OsStrExt;
        name.as_bytes().len()
    }
}

#[test]
fn a_temporary_name_is_hidden_and_named_after_its_target() {
    assert_eq!(temporary_name(OsStr::new("rapport.pdf"), "3f9a2c"), ".rapport.pdf.oxyde-3f9a2c");
}

#[test]
fn a_long_name_is_shortened_to_the_file_system_limit() {
    let long = "é".repeat(200);
    let name = temporary_name(OsStr::new(&long), "3f9a2c");
    assert!(units(&name) <= 255);
    let text = name.to_str().expect("whole characters are kept");
    assert!(text.starts_with(".é") && text.ends_with("é.oxyde-3f9a2c"));
}

#[test]
fn a_long_name_never_keeps_half_a_surrogate_pair() {
    let long = "😀".repeat(130);
    let name = temporary_name(OsStr::new(&long), "3f9a2c");
    assert!(units(&name) <= 255);
    assert!(name.to_str().is_some());
}
