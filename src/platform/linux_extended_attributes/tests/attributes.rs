use super::*;
use std::os::unix::fs::{MetadataExt, PermissionsExt};

/// An access ACL granting `nobody` read and write, in the kernel's xattr layout.
fn acl_with_named_user(mode: u32) -> Vec<u8> {
    let entries: [(u16, u16, u32); 5] = [
        (0x01, ((mode >> 6) & 7) as u16, u32::MAX),
        (0x02, 6, 65534),
        (0x04, ((mode >> 3) & 7) as u16, u32::MAX),
        (0x10, ((mode >> 3) & 7) as u16, u32::MAX),
        (0x20, (mode & 7) as u16, u32::MAX),
    ];
    let mut value = 2_u32.to_le_bytes().to_vec();
    for (tag, permissions, id) in entries {
        value.extend(tag.to_le_bytes());
        value.extend(permissions.to_le_bytes());
        value.extend(id.to_le_bytes());
    }
    value
}

fn names(attributes: &ExtendedAttributes) -> Vec<String> {
    attributes.0.iter().map(|(name, _)| name.to_string_lossy().into_owned()).collect()
}

#[test]
fn a_copied_file_keeps_user_attributes_and_its_acl_but_not_other_namespaces() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    std::fs::write(&source, b"data").unwrap();
    std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o640)).unwrap();
    rustix::fs::setxattr(&source, "user.xdg.origin.url", b"https://example.org", rustix::fs::XattrFlags::empty()).unwrap();
    let acl = acl_with_named_user(0o640);
    rustix::fs::setxattr(&source, "system.posix_acl_access", &acl, rustix::fs::XattrFlags::empty()).unwrap();
    let kept = read(&File::open(&source).unwrap()).unwrap();
    let mut kept_names = names(&kept);
    kept_names.sort();
    // An SELinux context, when present, stays behind.
    assert_eq!(kept_names, ["system.posix_acl_access", "user.xdg.origin.url"]);

    let destination = root.path().join("copy");
    super::super::linux_file_copy::copy(&source, &destination, None, &mut |_| Ok(()), &mut |_| Ok(())).unwrap();
    let copied = read(&File::open(&destination).unwrap()).unwrap();
    assert_eq!(copied.0, kept.0);
    assert_eq!(std::fs::metadata(&destination).unwrap().mode() & 0o7777, 0o640);
}

#[test]
fn a_refused_attribute_is_named_and_does_not_stop_the_others() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("file");
    std::fs::write(&path, b"").unwrap();
    let file = File::open(&path).unwrap();
    let attributes = ExtendedAttributes(vec![
        ("user.".into(), b"empty name".to_vec()),
        ("user.kept".into(), b"value".to_vec()),
    ]);
    let error = write(&file, &attributes).unwrap_err();
    assert!(error.to_string().contains("user.:"), "{error}");
    assert_eq!(names(&read(&file).unwrap()), ["user.kept"]);
}
