use super::*;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

#[test]
fn labels_lose_their_mnemonic_marks() {
    assert_eq!(without_mnemonics("_Open"), "Open");
    assert_eq!(without_mnemonics("Save__as_"), "Save_as");
    assert_eq!(without_mnemonics("Ouvrir"), "Ouvrir");
}

#[test]
fn paths_become_file_uris_byte_for_byte() {
    assert_eq!(uri(Path::new("/home/a/my file #1 (copy).txt")), "file:///home/a/my%20file%20%231%20(copy).txt");
    assert_eq!(uri(Path::new("/tmp/été?.md")), "file:///tmp/%C3%A9t%C3%A9%3F.md");
    let latin1 = std::ffi::OsStr::from_bytes(b"/tmp/\xe9t\xe9");
    assert_eq!(uri(Path::new(latin1)), "file:///tmp/%E9t%E9");
}

#[test]
fn results_serialize_as_the_portal_expects() {
    let mut values = Options::new();
    values.insert("uris".into(), OwnedValue::try_from(Value::from(vec!["file:///a".to_owned()])).unwrap());
    let (response, results) = reply(SUCCESS, values, Pending::start());
    assert_eq!(<(u32, Results) as Type>::SIGNATURE.to_string(), "(ua{sv})");
    let context = zbus::zvariant::serialized::Context::new_dbus(zbus::zvariant::LE, 0);
    let encoded = zbus::zvariant::to_bytes(context, &(response, &results)).unwrap();
    let (decoded, _): ((u32, Options), _) = encoded.deserialize().unwrap();
    assert_eq!(decoded.0, SUCCESS);
    assert_eq!(<Vec<String>>::try_from(decoded.1["uris"].try_clone().unwrap()).unwrap(), ["file:///a"]);
}

/// Options as they arrive over the bus, decoded from their bytes.
fn received(values: Options) -> Options {
    let context = zbus::zvariant::serialized::Context::new_dbus(zbus::zvariant::LE, 0);
    let encoded = zbus::zvariant::to_bytes(context, &values).unwrap();
    encoded.deserialize::<Options>().unwrap().0
}

fn owned<'a>(value: impl Into<Value<'a>>) -> OwnedValue {
    OwnedValue::try_from(value.into()).unwrap()
}

#[test]
fn filters_and_the_suggested_folder_are_read_from_the_options() {
    let images: RawFilter = ("Images".into(), vec![(0, "*.png".into()), (1, "image/jpeg".into())]);
    let text: RawFilter = ("Text".into(), vec![(1, "text/plain".into()), (7, "?".into())]);
    let folder = std::env::temp_dir();
    let mut bytes = std::os::unix::ffi::OsStrExt::as_bytes(folder.as_os_str()).to_vec();
    bytes.push(0);
    let mut values = Options::new();
    values.insert("filters".into(), owned(vec![images.clone(), text.clone()]));
    values.insert("current_filter".into(), owned(text.clone()));
    values.insert("current_folder".into(), owned(bytes));
    let options = received(values);
    let filters = raw_filters(&options);
    assert_eq!(filters, [images.clone(), text.clone()]);
    assert_eq!(options.get("current_filter").and_then(raw_filter), Some(text.clone()));
    assert_eq!(suggested_folder(&options), Some(folder));
    assert_eq!(type_filter(&text), FileTypeFilter { name: "Text".into(), patterns: vec![FileTypePattern::MimeType("text/plain".into())] });
    assert_eq!(type_filter(&images).patterns[0], FileTypePattern::Glob("*.png".into()));
    // A folder that does not exist is no suggestion.
    let mut missing = Options::new();
    missing.insert("current_folder".into(), owned(b"/no/such/folder\0".to_vec()));
    assert_eq!(suggested_folder(&received(missing)), None);
}

#[test]
fn the_current_filter_is_found_given_alone_or_added() {
    let a: RawFilter = ("A".into(), vec![(0, "*.a".into())]);
    let b: RawFilter = ("B".into(), vec![(0, "*.b".into())]);
    assert_eq!(offered_filters(vec![a.clone(), b.clone()], Some(b.clone())), (vec![a.clone(), b.clone()], Some(1)));
    assert_eq!(offered_filters(vec![a.clone(), b.clone()], None), (vec![a.clone(), b.clone()], Some(0)));
    assert_eq!(offered_filters(Vec::new(), Some(b.clone())), (vec![b.clone()], Some(0)));
    assert_eq!(offered_filters(vec![a.clone()], Some(b.clone())), (vec![a, b], Some(1)));
    assert_eq!(offered_filters(Vec::new(), None), (Vec::new(), None));
}
