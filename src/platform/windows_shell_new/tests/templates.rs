use super::*;
use windows::Win32::System::Registry::{
    HKEY_CURRENT_USER, KEY_WRITE, REG_BINARY, REG_OPTION_VOLATILE, REG_SZ, REG_VALUE_TYPE,
    RegCreateKeyExW, RegDeleteTreeW, RegSetValueExW,
};

struct RegistryFixture {
    parent: String,
    root: Key,
}

impl RegistryFixture {
    fn new() -> Self {
        let parent = format!(r"Software\Oxyde-Test-ShellNew-{}", uuid::Uuid::new_v4());
        let root = create_key(HKEY_CURRENT_USER, &parent);
        Self { parent, root }
    }

    fn key(&self, path: &str) -> Key {
        create_key(self.root.0, path)
    }

    fn template(&self, extension: &str) -> Option<ShellNewTemplate> {
        find_template(self.root.0, extension.to_owned())
    }
}

impl Drop for RegistryFixture {
    fn drop(&mut self) {
        let result =
            unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, PCWSTR(wide(&self.parent).as_ptr())) };
        assert_eq!(result, ERROR_SUCCESS);
    }
}

fn create_key(root: HKEY, name: &str) -> Key {
    let mut key = HKEY::default();
    registry_result(unsafe {
        RegCreateKeyExW(
            root,
            PCWSTR(wide(name).as_ptr()),
            Some(0),
            None,
            REG_OPTION_VOLATILE,
            KEY_READ | KEY_WRITE,
            None,
            &mut key,
            None,
        )
    })
    .unwrap();
    Key(key)
}

fn value(key: &Key, name: PCWSTR, kind: REG_VALUE_TYPE, data: &[u8]) {
    registry_result(unsafe { RegSetValueExW(key.0, name, Some(0), kind, Some(data)) }).unwrap();
}

fn text(key: &Key, name: PCWSTR, data: &str) {
    let bytes: Vec<_> = wide(data).into_iter().flat_map(u16::to_le_bytes).collect();
    value(key, name, REG_SZ, &bytes);
}

#[test]
fn reads_direct_and_progid_registrations_and_uses_windows_value_precedence() {
    let registry = RegistryFixture::new();
    text(&registry.key(".oxyde-test"), PCWSTR::null(), "Oxyde.Test.1");
    text(
        &registry.key("Oxyde.Test.1"),
        PCWSTR::null(),
        "Oxyde document",
    );
    let registration = registry.key(r".oxyde-test\Oxyde.Test.1\ShellNew");
    value(&registration, w!("Data"), REG_BINARY, b"model bytes");
    let template = registry.template(".oxyde-test").unwrap();
    assert_eq!(template.label, "Oxyde document");
    assert!(
        matches!(template.contents, Contents::RegistryData(path) if path == r".oxyde-test\Oxyde.Test.1\ShellNew")
    );
    assert_eq!(
        registry_data(registry.root.0, r".oxyde-test\Oxyde.Test.1\ShellNew").unwrap(),
        b"model bytes"
    );

    text(&registration, w!("FileName"), "model.oxyde-test");
    assert!(matches!(
        registry.template(".oxyde-test").unwrap().contents,
        Contents::File(_)
    ));
    text(&registration, w!("NullFile"), "");
    assert!(matches!(
        registry.template(".oxyde-test").unwrap().contents,
        Contents::Empty
    ));

    let direct = registry.key(r".oxyde-test\ShellNew");
    text(&direct, w!("MenuText"), "Direct entry");
    text(&direct, w!("FileName"), "direct.oxyde-test");
    let template = registry.template(".oxyde-test").unwrap();
    assert_eq!(template.label, "Direct entry");
    assert!(
        matches!(template.contents, Contents::File(path) if path == Path::new("direct.oxyde-test"))
    );

    text(&direct, w!("Command"), "handled by Windows");
    assert!(registry.template(".oxyde-test").is_none());
}

#[test]
fn creates_an_empty_file_or_copies_a_model_without_overwriting() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.bin");
    std::fs::write(&source, b"model contents").unwrap();
    let occupied = dir.path().join("New file.test");
    std::fs::write(&occupied, b"existing contents").unwrap();
    let mut template = ShellNewTemplate {
        label: "Test".into(),
        extension: ".test".into(),
        contents: Contents::File(source),
    };

    let created = create_file(
        dir.path(),
        &template,
        &mut ["New file", "New file (1)"].into_iter().map(String::from),
    )
    .unwrap();
    assert_eq!(created, dir.path().join("New file (1).test"));
    assert_eq!(std::fs::read(created).unwrap(), b"model contents");
    assert_eq!(std::fs::read(occupied).unwrap(), b"existing contents");

    template.contents = Contents::Empty;
    let empty = create_file(dir.path(), &template, &mut ["Empty".to_owned()].into_iter()).unwrap();
    assert_eq!(std::fs::metadata(empty).unwrap().len(), 0);
}

#[test]
fn rejects_non_local_names_and_leaves_no_file_when_a_model_is_missing() {
    let dir = tempfile::tempdir().unwrap();
    let mut template = ShellNewTemplate {
        label: "Test".into(),
        extension: ".test".into(),
        contents: Contents::Empty,
    };
    for name in [r"..\elsewhere", "file:stream", "trailing."] {
        let error =
            create_file(dir.path(), &template, &mut [name.to_owned()].into_iter()).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }

    template.contents = Contents::File(dir.path().join("missing.test"));
    assert!(
        create_file(
            dir.path(),
            &template,
            &mut ["New file".to_owned()].into_iter()
        )
        .is_err()
    );
    assert_eq!(dir.path().read_dir().unwrap().count(), 0);
}
