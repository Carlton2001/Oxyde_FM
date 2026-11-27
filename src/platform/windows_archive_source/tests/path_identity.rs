use super::path_identity;

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

#[test]
fn same_path_gives_the_same_identity() {
    let path = wide(r"\\?\UNC\localhost@8088\DavWWWRoot\Dossier\a.txt");
    let identity = path_identity(&path).unwrap();
    assert_eq!(identity, path_identity(&path).unwrap());
    assert_eq!(identity.kind, 4);
}

#[test]
fn names_differing_by_case_stay_distinct() {
    let lower = path_identity(&wide(r"\\?\UNC\server\share\fichier.txt")).unwrap();
    let upper = path_identity(&wide(r"\\?\UNC\server\share\Fichier.txt")).unwrap();
    assert_ne!(lower, upper);
    assert_eq!(lower.volume, upper.volume);
}

#[test]
fn shares_have_distinct_volumes_and_roots() {
    let first = path_identity(&wide(r"\\?\UNC\server\first\")).unwrap();
    let second = path_identity(&wide(r"\\?\UNC\server\second\")).unwrap();
    assert_ne!(first.volume, second.volume);
    assert_ne!(first, second);
    assert_eq!(first, path_identity(&wide(r"\\?\UNC\server\first")).unwrap());
}

#[test]
fn local_and_malformed_paths_have_none() {
    assert!(path_identity(&wide(r"\\?\C:\Users\file.txt")).is_none());
    assert!(path_identity(&wide(r"\\?\UNC\server")).is_none());
    assert!(path_identity(&wide(r"\\?\UNC\\share")).is_none());
}
