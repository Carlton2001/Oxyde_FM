use super::*;

#[test]
fn user_dpapi_blob_or_another_application_key_cannot_forge_a_cache_proof() {
    let fixture = tempfile::tempdir().unwrap();
    let path = fixture.path().join(".cache-proof-key");
    let secret = key::load(&path).unwrap();
    let loaded = key::load(&path).unwrap();
    assert!(secret == loaded);
    let signed = transform(b"creation proof", &secret, true).unwrap();
    assert_eq!(
        transform(&signed, &loaded, false).unwrap(),
        b"creation proof"
    );
    let user_blob = dpapi(b"forged creation proof", true).unwrap();
    assert!(transform(&user_blob, &secret, false).is_err());
    let forged = transform(b"forged creation proof", &[0; 32], true).unwrap();
    assert!(transform(&forged, &secret, false).is_err());
}

#[test]
fn unknown_key_file_is_preserved_and_never_reinitialized_by_its_reserved_name() {
    let fixture = tempfile::tempdir().unwrap();
    let path = fixture.path().join(".cache-proof-key");
    std::fs::write(&path, b"foreign document").unwrap();
    assert!(key::load(&path).is_err());
    assert_eq!(std::fs::read(path).unwrap(), b"foreign document");
}

#[test]
fn signing_key_lookup_refuses_an_ancestor_junction_without_touching_its_target() {
    let fixture = tempfile::tempdir().unwrap();
    let outside = fixture.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    let foreign = outside.join(".cache-proof-key");
    std::fs::write(&foreign, b"external key witness").unwrap();
    let link = fixture.path().join("linked");
    crate::platform::windows_reparse_tests::junction(&link, &outside);
    assert!(key::load(&link.join(".cache-proof-key")).is_err());
    assert_eq!(std::fs::read(foreign).unwrap(), b"external key witness");
    std::fs::remove_dir(link).unwrap();
}
