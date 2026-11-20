use super::*;

fn touch(folder: &Path, names: &[&str]) {
    for name in names {
        std::fs::write(folder.join(name), b"x").unwrap();
    }
}

#[test]
fn new_naming_lists_consecutive_parts_from_any_of_them() {
    let folder = tempfile::tempdir().unwrap();
    touch(folder.path(), &["film.part01.rar", "film.part02.rar", "film.part03.rar", "film.part05.rar"]);
    let expected: Vec<_> = ["film.part01.rar", "film.part02.rar", "film.part03.rar"]
        .iter()
        .map(|name| folder.path().join(name))
        .collect();
    assert_eq!(rar_volumes(&folder.path().join("film.part01.rar")), expected);
    assert_eq!(rar_volumes(&folder.path().join("film.part02.rar")), expected);
    assert!(is_rar_volume_set(&folder.path().join("film.part03.rar")));
}

#[test]
fn old_naming_follows_the_rar_with_r00_onwards() {
    let folder = tempfile::tempdir().unwrap();
    touch(folder.path(), &["Data.RAR", "Data.R00", "Data.R01"]);
    assert_eq!(
        rar_volumes(&folder.path().join("Data.RAR")),
        ["Data.RAR", "Data.R00", "Data.R01"].iter().map(|name| folder.path().join(name)).collect::<Vec<_>>()
    );
}

#[test]
fn single_archives_and_orphan_volumes_stay_alone() {
    let folder = tempfile::tempdir().unwrap();
    touch(folder.path(), &["plain.rar", "lone.part1.rar", "orphan.part2.rar"]);
    for name in ["plain.rar", "lone.part1.rar", "orphan.part2.rar"] {
        let path = folder.path().join(name);
        assert_eq!(rar_volumes(&path), vec![path.clone()]);
        assert!(!is_rar_volume_set(&path));
    }
    let other = folder.path().join("notes.zip");
    assert_eq!(rar_volumes(&other), vec![other.clone()]);
}

#[test]
fn set_names_lead_every_part_to_the_first_without_touching_files() {
    assert_eq!(
        rar_set_name(Path::new("/d/film.part03.rar")),
        Some((PathBuf::from("/d/film.part01.rar"), "film".to_owned()))
    );
    assert_eq!(rar_set_name(Path::new("/d/Film.PART2.RAR")).map(|(_, name)| name), Some("Film".to_owned()));
    assert_eq!(rar_set_name(Path::new("/d/plain.rar")), None);
    assert_eq!(rar_set_name(Path::new("/d/a.part.rar")), None);
}

#[test]
fn old_naming_continues_past_r99_with_s00() {
    let folder = tempfile::tempdir().unwrap();
    let mut names = vec!["big.rar".to_owned()];
    names.extend((0..100).map(|number| format!("big.r{number:02}")));
    names.extend(["big.s00".to_owned(), "big.s01".to_owned()]);
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    touch(folder.path(), &names);
    let volumes = rar_volumes(&folder.path().join("big.rar"));
    assert_eq!(volumes.len(), 103);
    assert_eq!(volumes.last().unwrap(), &folder.path().join("big.s01"));
}
