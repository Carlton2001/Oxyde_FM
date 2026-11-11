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
    assert_eq!(first_folder(suggestions(&options)), Some(folder));
    assert_eq!(type_filter(&text), FileTypeFilter { name: "Text".into(), patterns: vec![FileTypePattern::MimeType("text/plain".into())] });
    assert_eq!(type_filter(&images).patterns[0], FileTypePattern::Glob("*.png".into()));
    // A folder that does not exist is no suggestion.
    let mut missing = Options::new();
    missing.insert("current_folder".into(), owned(b"/no/such/folder\0".to_vec()));
    assert_eq!(first_folder(suggestions(&received(missing))), None);
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

fn nul_terminated(path: &Path) -> Vec<u8> {
    let mut bytes = path.as_os_str().as_bytes().to_vec();
    bytes.push(0);
    bytes
}

#[test]
fn saving_reads_the_name_the_folder_and_the_names_of_several_files() {
    let folder = std::env::temp_dir();
    let mut values = Options::new();
    values.insert("current_name".into(), owned("Untitled.txt"));
    values.insert("current_folder".into(), owned(nul_terminated(Path::new("/no/such/folder"))));
    let options = received(values);
    assert_eq!(purpose("SaveFile", &options), FileChooserPurpose::Save { name: "Untitled.txt".into() });
    assert_eq!(first_folder(suggestions(&options)), None);
    // The file saved again gives its name and its folder.
    let mut values = Options::new();
    values.insert("current_name".into(), owned("Untitled.txt"));
    values.insert("current_file".into(), owned(nul_terminated(&folder.join("report.odt"))));
    let options = received(values);
    assert_eq!(purpose("SaveFile", &options), FileChooserPurpose::Save { name: "report.odt".into() });
    assert_eq!(first_folder(suggestions(&options)), Some(folder.clone()));
    assert_eq!(purpose("SaveFile", &Options::new()), FileChooserPurpose::Save { name: String::new() });
    // Several files keep their names' last component, whatever their bytes.
    let mut values = Options::new();
    values.insert("files".into(), owned(vec![b"a.png\0".to_vec(), b"sub/b.png\0".to_vec(), b"\xe9.txt\0".to_vec(), b"..\0".to_vec()]));
    let names = match purpose("SaveFiles", &received(values)) {
        FileChooserPurpose::SaveFiles { names } => names,
        other => panic!("{other:?}"),
    };
    let latin1 = std::ffi::OsStr::from_bytes(b"\xe9.txt").to_owned();
    assert_eq!(names, [std::ffi::OsString::from("a.png"), "b.png".into(), latin1]);
    assert_eq!(purpose("OpenFile", &Options::new()), FileChooserPurpose::Open);
}

#[test]
fn choices_are_read_and_their_first_answer_checked() {
    let encoding: RawChoice = ("encoding".into(), "_Encoding".into(),
        vec![("utf8".into(), "Unicode".into()), ("latin15".into(), "Western".into())], "latin15".into());
    let mut values = Options::new();
    values.insert("choices".into(), owned(vec![encoding.clone(), ("reencode".into(), "Re-encode".into(), Vec::new(), "true".into())]));
    let choices: Vec<FileChooserChoice> = raw_choices(&received(values)).into_iter().map(choice).collect();
    assert_eq!(choices[0], FileChooserChoice {
        id: "encoding".into(),
        label: "Encoding".into(),
        options: vec![("utf8".into(), "Unicode".into()), ("latin15".into(), "Western".into())],
        initial: "latin15".into(),
    });
    assert_eq!(choices[1].initial, "true");
    let unknown = (encoding.0.clone(), encoding.1.clone(), encoding.2.clone(), "koi8".into());
    assert_eq!(choice(unknown).initial, "utf8");
    assert_eq!(choice(("box".into(), "Box".into(), Vec::new(), "yes".into())).initial, "false");
}

#[test]
fn parent_windows_are_read_in_both_forms() {
    assert_eq!(parent_window("x11:3a0000b"), Some(FileChooserParent::X11(0x3a0000b)));
    assert_eq!(parent_window("x11:0x3a0000b"), Some(FileChooserParent::X11(0x3a0000b)));
    assert_eq!(parent_window("wayland:7c1f2e9a-handle"), Some(FileChooserParent::Wayland("7c1f2e9a-handle".into())));
    for nothing in ["", "x11:", "x11:0", "x11:zz", "x11:1ffffffff", "wayland:", "mir:12"] {
        assert_eq!(parent_window(nothing), None, "{nothing:?}");
    }
}
