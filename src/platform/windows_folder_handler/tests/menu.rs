use super::{installed_in, listed, values};

const PROGRAM: &str = r"C:\Users\Me\AppData\Local\Programs\Oxyde_FM\Oxyde_FM.exe";

#[test]
fn folders_their_background_and_drives_get_a_verb_running_oxyde_on_them() {
    let values = values(PROGRAM, Some("Open in Oxyde"));
    for class in ["Directory", r"Directory\Background", "Drive"] {
        let data = |key: &str, name: &str| values.iter().find(|value| value.key == key && value.name == name).and_then(|value| value.data.clone());
        let verb = format!(r"{class}\shell\Oxyde");
        assert_eq!(data(&verb, "").as_deref(), Some("Open in Oxyde"));
        assert_eq!(data(&verb, "Icon").as_deref(), Some(PROGRAM));
        assert_eq!(data(&format!(r"{verb}\command"), ""), Some(format!("\"{PROGRAM}\" \"%V\"")));
    }
}

#[test]
fn without_a_label_every_value_is_removed() {
    let listed = values(PROGRAM, Some("Open in Oxyde"));
    let removed = values(PROGRAM, None);
    assert_eq!(removed.len(), listed.len());
    assert!(removed.iter().all(|value| value.data.is_none()));
}

#[test]
fn the_setup_records_its_folder_with_a_trailing_separator() {
    let folder = r"C:\Users\Me\AppData\Local\Programs\Oxyde_FM";
    assert!(installed_in(Some(r"c:\users\me\appdata\local\programs\oxyde_fm\"), folder));
    assert!(!installed_in(Some(r"D:\Portable\Oxyde_FM\"), folder));
    assert!(!installed_in(None, folder));
}

#[test]
fn only_the_installed_copy_is_listed_and_only_while_it_is_not_the_default() {
    assert!(listed(true, Some(false)));
    assert!(!listed(true, Some(true)));
    assert!(!listed(true, None));
    assert!(!listed(false, Some(false)));
}
