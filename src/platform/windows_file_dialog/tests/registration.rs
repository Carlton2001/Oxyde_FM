use super::{names, ours, values};

const LIBRARY: &str = r"C:\Users\Me\AppData\Local\Programs\Oxyde_FM\oxyde_file_dialog.dll";

#[test]
fn both_dialogs_are_served_by_the_library_in_the_thread_that_creates_them() {
    let values = values(Some(LIBRARY));
    for dialog in ["{DC1C5A9C-E88A-4DDE-A5A1-60F82A20AEF7}", "{C0B4E2F3-BA21-4773-8DBA-335EC946EB8B}"] {
        let key = format!(r"CLSID\{dialog}\InprocServer32");
        let data = |name: &str| values.iter().find(|value| value.key == key && value.name == name).and_then(|value| value.data.clone());
        assert_eq!(data("").as_deref(), Some(LIBRARY));
        assert_eq!(data("ThreadingModel").as_deref(), Some("Apartment"));
    }
    assert_eq!(values.len(), 4);
}

#[test]
fn giving_the_dialogs_back_removes_every_value() {
    let removed = values(None);
    assert_eq!(removed.len(), 4);
    assert!(removed.iter().all(|value| value.data.is_none()));
}

#[test]
fn a_registration_is_this_copys_another_copys_or_another_programs() {
    assert!(names(Some(&LIBRARY.to_uppercase()), LIBRARY));
    assert!(!names(Some(r"D:\Oxyde\oxyde_file_dialog.dll"), LIBRARY));
    assert!(!names(None, LIBRARY));
    assert!(ours(r"D:\Oxyde\OXYDE_FILE_DIALOG.DLL"));
    assert!(!ours(r"C:\Program Files\Files\Files.App.OpenDialog.dll"));
    assert!(!ours(r"C:\Windows\System32\comdlg32.dll"));
}
