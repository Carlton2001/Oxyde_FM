use super::*;

const SYSTEM: &str = "[preferred]\ndefault=gnome;gtk;\n";

fn places(root: &Path) -> Places {
    Places {
        config_home: root.join("home/.config"),
        config_dirs: vec![root.join("etc/xdg")],
        data_home: root.join("home/.local/share"),
        data_dirs: vec![root.join("usr/local/share")],
        built_in: [root.join("etc"), root.join("usr/share")],
        desktops: vec!["ubuntu".into(), "gnome".into()],
    }
}

fn put(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

#[test]
fn the_first_folder_with_a_configuration_decides_as_the_portal_reads_it() {
    let root = tempfile::tempdir().unwrap();
    let places = places(root.path());
    assert_eq!(places.config(), None);
    put(&root.path().join("usr/share/xdg-desktop-portal/gnome-portals.conf"), SYSTEM);
    assert_eq!(places.config(), Some(root.path().join("usr/share/xdg-desktop-portal/gnome-portals.conf")));
    // A desktop named earlier wins within a folder; a user's generic file wins over all.
    put(&root.path().join("usr/share/xdg-desktop-portal/ubuntu-portals.conf"), SYSTEM);
    assert_eq!(places.config(), Some(root.path().join("usr/share/xdg-desktop-portal/ubuntu-portals.conf")));
    assert_eq!(places.user_config(), root.path().join("home/.config/xdg-desktop-portal/ubuntu-portals.conf"));
    put(&root.path().join("home/.config/xdg-desktop-portal/portals.conf"), SYSTEM);
    assert_eq!(places.config(), Some(root.path().join("home/.config/xdg-desktop-portal/portals.conf")));
    assert_eq!(places.user_config(), root.path().join("home/.config/xdg-desktop-portal/portals.conf"));
}

#[test]
fn a_copy_of_the_system_configuration_comes_and_goes_with_oxyde() {
    let root = tempfile::tempdir().unwrap();
    let places = places(root.path());
    let system = root.path().join("usr/share/xdg-desktop-portal/gnome-portals.conf");
    put(&system, SYSTEM);
    edit_config(&places, true).unwrap();
    let user = root.path().join("home/.config/xdg-desktop-portal/ubuntu-portals.conf");
    let written = std::fs::read_to_string(&user).unwrap();
    assert!(written.starts_with(&format!("{}{}\n", conf::CREATED, system.display())), "{written}");
    assert!(places.named());
    edit_config(&places, false).unwrap();
    assert!(!user.exists());
    assert!(!places.named());
}

#[test]
fn a_copy_the_user_changed_since_stays_without_oxyde() {
    let root = tempfile::tempdir().unwrap();
    let places = places(root.path());
    put(&root.path().join("usr/share/xdg-desktop-portal/gnome-portals.conf"), SYSTEM);
    edit_config(&places, true).unwrap();
    let user = places.user_config();
    let changed = std::fs::read_to_string(&user).unwrap() + "org.freedesktop.impl.portal.Secret=gnome-keyring;\n";
    std::fs::write(&user, &changed).unwrap();
    edit_config(&places, false).unwrap();
    let kept = std::fs::read_to_string(&user).unwrap();
    assert!(kept.contains("Secret=gnome-keyring;") && !kept.contains(ID), "{kept}");
}

#[test]
fn the_users_own_file_keeps_everything_but_oxyde() {
    let root = tempfile::tempdir().unwrap();
    let places = places(root.path());
    let user = root.path().join("home/.config/xdg-desktop-portal/gnome-portals.conf");
    let text = "[preferred]\ndefault=gnome;gtk;\norg.freedesktop.impl.portal.FileChooser=strata;gnome;gtk;\n";
    put(&user, text);
    edit_config(&places, true).unwrap();
    assert_eq!(conf::portals(&std::fs::read_to_string(&user).unwrap()), [ID, "strata", "gnome", "gtk"]);
    edit_config(&places, false).unwrap();
    assert_eq!(std::fs::read_to_string(&user).unwrap(), text);
    // Turning it off with no file of the user's writes none.
    std::fs::remove_file(&user).unwrap();
    edit_config(&places, false).unwrap();
    assert!(!user.exists());
}

#[test]
fn a_linked_configuration_stays_a_link() {
    let root = tempfile::tempdir().unwrap();
    let places = places(root.path());
    let target = root.path().join("dotfiles/gnome-portals.conf");
    put(&target, SYSTEM);
    let link = root.path().join("home/.config/xdg-desktop-portal/gnome-portals.conf");
    std::fs::create_dir_all(link.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&target, &link).unwrap();
    edit_config(&places, true).unwrap();
    assert!(std::fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
    assert_eq!(conf::portals(&std::fs::read_to_string(&target).unwrap())[0], ID);
}

#[test]
fn portal_files_name_their_bus_for_file_choosing_only() {
    assert_eq!(backend_name("[portal]\nDBusName=org.freedesktop.impl.portal.desktop.gnome\nInterfaces=org.freedesktop.impl.portal.Access;org.freedesktop.impl.portal.FileChooser;\n"),
        Some("org.freedesktop.impl.portal.desktop.gnome".into()));
    assert_eq!(backend_name("[portal]\nDBusName=org.freedesktop.impl.portal.desktop.oo7\nInterfaces=org.freedesktop.impl.portal.Secret;\n"), None);
    assert_eq!(portal_file(), format!("[portal]\nDBusName={NAME}\nInterfaces=org.freedesktop.impl.portal.FileChooser;\n"));
}
