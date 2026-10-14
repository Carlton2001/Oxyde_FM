use super::*;

#[test]
fn ordinary_files_share_sources_by_extension_without_touching_disk() {
    assert_eq!(
        source(r"C:\one\A.TXT", false, false),
        source("D:/two/b.txt", false, false)
    );
    assert_eq!(source("missing.pdf", false, false), "oxyde_ext_pdf");
    assert_ne!(source("a.pdf", false, false), source("a.txt", false, false));
    assert_eq!(
        source(r"C:\folder.txt\README", false, false),
        "oxyde_file_generic"
    );
    assert_eq!(
        source("file.", false, false),
        source("README", false, false)
    );
    assert_eq!(source("oxyde_ext_txt", false, false), "oxyde_ext_txt");
}

#[test]
fn custom_icons_and_directories_remain_individual() {
    for extension in UNIQUE_EXTENSIONS {
        let path = format!(r"C:\apps\one.{}", extension.to_ascii_uppercase());
        assert_eq!(source(&path, false, false), path);
        assert_ne!(
            source(&path, false, false),
            source(&format!("two.{extension}"), false, false)
        );
    }
    for path in [
        r"C:\folder.txt",
        r"C:\",
        r"\\server",
        r"C:\Users\User\Documents",
    ] {
        assert_eq!(source(path, true, true), path);
    }
}

#[test]
fn ordinary_folders_share_one_source_and_known_locations_keep_their_icons() {
    for path in [
        r"C:\work\one",
        r"D:\work\two",
        r"C:\folder.txt",
        "C:/work/three",
    ] {
        assert_eq!(source(path, true, false), "oxyde_dir_generic");
    }
    for path in [
        r"C:\",
        r"\\server",
        r"\\server\share",
        r"C:\Users\User\Documents",
        r"C:\Users\User\Downloads\",
    ] {
        assert_eq!(source(path, true, false), path);
    }
    assert_eq!(source(r"C:\work\junction", true, true), r"C:\work\junction");
}
