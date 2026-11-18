use super::*;

fn file(name: &str, size: u64, modified: u64) -> FileEntry {
    FileEntry { name: name.into(), path: format!("/f/{name}"), size, modified, ..Default::default() }
}

#[test]
fn a_type_holds_while_the_file_keeps_its_date_and_size() {
    let mut cache = TypeCache::default();
    let entries = vec![file("a", 1, 10), file("b", 2, 20), FileEntry { is_dir: true, ..file("c", 0, 0) }];
    let (known, pending) = cache.split(&entries);
    assert!(known.is_empty());
    assert_eq!(pending, vec![("/f/a".into(), (10, 1)), ("/f/b".into(), (20, 2))]);
    cache.remember(&[(pending[0].clone(), "text/plain"), (pending[1].clone(), "image/png")]);
    let changed = vec![file("a", 1, 10), file("b", 3, 20)];
    let (known, pending) = cache.split(&changed);
    assert_eq!(known, HashMap::from([("/f/a".to_string(), "text/plain")]));
    assert_eq!(pending, vec![("/f/b".into(), (20, 3))]);
}
