use super::{open_command, replaced, restored, values, Value};

const PROGRAM: &str = r"C:\Programs\Oxyde\oxyde-folder-launcher.exe";

fn absent(value: &Value) -> Value {
    Value { data: None, expand: false, ..value.clone() }
}

#[test]
fn folders_and_drives_get_an_open_verb_that_no_delegate_overrides() {
    let values = values(PROGRAM);
    for class in ["Directory", "Drive"] {
        let find = |key: String, name: &str| values.iter().find(|value| value.key == key && value.name == name).and_then(|value| value.data.clone());
        assert_eq!(find(format!(r"{class}\shell"), "").as_deref(), Some("open"));
        assert_eq!(find(format!(r"{class}\shell\open\command"), "").as_deref(), Some(r#""C:\Programs\Oxyde\oxyde-folder-launcher.exe" "%V""#));
        assert_eq!(find(format!(r"{class}\shell\open\command"), "DelegateExecute").as_deref(), Some(""));
    }
    assert!(values.iter().all(|value| !value.key.starts_with("Folder")));
}

#[test]
fn values_found_are_kept_and_those_already_of_oxyde_stand_for_none() {
    let wanted = values(PROGRAM);
    let before = replaced(None, &wanted, |value| match (value.key.as_str(), value.name.as_str()) {
        (r"Directory\shell", "") => Value { data: Some("other".into()), ..absent(value) },
        (r"Directory\shell\open\command", "") => Value { data: Some(open_command(PROGRAM)), ..absent(value) },
        (r"Drive\shell\open\command", "") => Value { data: Some(r"%SystemRoot%\other.exe".into()), expand: true, ..absent(value) },
        _ => absent(value),
    });
    assert_eq!(before.len(), wanted.len());
    assert_eq!(before[0].data.as_deref(), Some("other"));
    assert_eq!(before[1].data, None);
    assert_eq!((before[4].data.as_deref(), before[4].expand), (Some(r"%SystemRoot%\other.exe"), true));
}

#[test]
fn values_kept_at_the_last_change_win_over_those_read_now() {
    let wanted = values(PROGRAM);
    let mut kept: Vec<Value> = wanted.iter().map(absent).collect();
    kept[0].data = Some("other".into());
    let text = serde_json::to_string(&kept).unwrap();
    let moved = values(r"D:\Oxyde\oxyde-folder-launcher.exe");
    let read_now = |value: &Value| moved.iter().find(|moved| moved.key == value.key && moved.name == value.name).unwrap().clone();
    assert_eq!(replaced(Some(&text), &wanted, read_now), kept);
    assert_eq!(restored(Some(&text), &wanted), kept);
}

#[test]
fn without_kept_values_turning_off_removes_every_value() {
    let wanted = values(PROGRAM);
    for kept in [None, Some("org.gnome.Nautilus.desktop"), Some("[]")] {
        let restored = restored(kept, &wanted);
        assert_eq!(restored.len(), wanted.len());
        assert!(restored.iter().all(|value| value.data.is_none()));
    }
}
