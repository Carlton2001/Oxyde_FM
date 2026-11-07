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
