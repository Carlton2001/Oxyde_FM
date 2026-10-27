use super::bookmarks;

const NAUTILUS: &str = "file:///home/user/Projects Projects
file:///home/user/Vid%C3%A9os Vidéos
sftp://server/srv Serveur
file:///home/user/Mes%20documents
";

#[test]
fn local_bookmarks_keep_their_names_and_order() {
    let folder = tempfile::tempdir().unwrap();
    let file = folder.path().join("gtk-3.0/bookmarks");
    assert!(bookmarks::items(&file).is_empty());
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, NAUTILUS).unwrap();
    let items: Vec<_> = bookmarks::items(&file).into_iter().map(|item| (item.name, item.path)).collect();
    assert_eq!(
        items,
        [
            ("Projects".to_string(), "/home/user/Projects".to_string()),
            ("Vidéos".to_string(), "/home/user/Vidéos".to_string()),
            ("Mes documents".to_string(), "/home/user/Mes documents".to_string()),
        ]
    );
}

#[test]
fn pinning_appends_once_and_unpinning_keeps_other_lines() {
    let folder = tempfile::tempdir().unwrap();
    let file = folder.path().join("gtk-3.0/bookmarks");
    bookmarks::pin(&file, "/home/user/Été 2026 #1").unwrap();
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "file:///home/user/%C3%89t%C3%A9%202026%20%231\n");
    std::fs::write(&file, NAUTILUS).unwrap();
    bookmarks::pin(&file, "/home/user/Vidéos").unwrap();
    assert_eq!(std::fs::read_to_string(&file).unwrap(), NAUTILUS);
    bookmarks::pin(&file, "/srv/(data)").unwrap();
    bookmarks::unpin(&file, "/home/user/Projects").unwrap();
    bookmarks::unpin(&file, "/home/user/absent").unwrap();
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "file:///home/user/Vid%C3%A9os Vidéos\nsftp://server/srv Serveur\nfile:///home/user/Mes%20documents\nfile:///srv/(data)\n"
    );
    let leftovers = std::fs::read_dir(file.parent().unwrap()).unwrap().count();
    assert_eq!(leftovers, 1);
}
