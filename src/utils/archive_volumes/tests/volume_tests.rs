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

#[test]
fn a_later_old_naming_volume_leads_to_its_first_volume() {
    let folder = tempfile::tempdir().unwrap();
    touch(folder.path(), &["film.rar", "film.r00", "film.r01", "film.r03", "Data.RAR", "Data.R00", "lone.r00"]);
    let path = |name: &str| folder.path().join(name);
    assert_eq!(first_volume_of_old_naming(&path("film.r01")), Some(path("film.rar")));
    assert_eq!(first_volume_of_old_naming(&path("Data.R00")), Some(path("Data.RAR")));
    // After a missing volume, or without a first one, the file is no volume of a set.
    assert_eq!(first_volume_of_old_naming(&path("film.r03")), None);
    assert_eq!(first_volume_of_old_naming(&path("lone.r00")), None);
}

#[test]
fn only_old_naming_extensions_name_a_first_volume() {
    assert_eq!(old_naming_first_volume(Path::new("/d/film.s42")), Some(PathBuf::from("/d/film.rar")));
    assert_eq!(old_naming_first_volume(Path::new("/d/film.Z99")), Some(PathBuf::from("/d/film.RAR")));
    for name in ["/d/film.rar", "/d/film.q00", "/d/film.r0", "/d/film.r001", "/d/.r00", "/d/script.r"] {
        assert_eq!(old_naming_first_volume(Path::new(name)), None, "{name}");
    }
}
