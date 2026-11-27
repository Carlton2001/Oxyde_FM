use super::*;

fn leaf(id: i32, label: &str, verb: Option<&str>) -> WinMenuItem {
    WinMenuItem {
        id,
        label: label.into(),
        verb: verb.map(str::to_owned),
        has_submenu: false,
        is_folder_sharing_command: false,
        children: Vec::new(),
    }
}

fn submenu(label: &str, children: Vec<WinMenuItem>) -> WinMenuItem {
    WinMenuItem {
        has_submenu: true,
        children,
        ..leaf(-1, label, None)
    }
}

#[test]
fn cloud_files_uses_the_native_verb_even_with_duplicate_labels() {
    let items = [
        leaf(7, "Toujours conserver sur cet appareil", None),
        leaf(
            20,
            "Toujours conserver sur cet appareil",
            Some("MakeAvailableOffline"),
        ),
        leaf(
            30,
            "Toujours conserver sur cet appareil",
            Some("MakeAvailableOffline"),
        ),
        leaf(50, "Libérer de l’espace", Some("MakeAvailableOnline")),
    ];
    assert_eq!(
        cloud_files_command(&items, CloudAvailability::Offline)
            .unwrap()
            .id,
        20
    );
    assert_eq!(
        cloud_files_command(&items, CloudAvailability::OnlineOnly)
            .unwrap()
            .id,
        50
    );
}

#[test]
fn google_uses_the_direction_within_its_offline_access_submenu() {
    let items = [
        leaf(4, "En ligne uniquement", None),
        submenu(
            "Accès hors connexion",
            vec![
                leaf(18, "En ligne uniquement", None),
                leaf(19, "Disponible hors connexion", None),
            ],
        ),
    ];
    assert_eq!(
        google_command(&items, CloudAvailability::Offline)
            .unwrap()
            .id,
        19
    );
    assert_eq!(
        google_command(&items, CloudAvailability::OnlineOnly)
            .unwrap()
            .id,
        18
    );
}

#[test]
fn google_rejects_ambiguous_or_missing_provider_commands() {
    let items = [
        submenu("Offline access", vec![leaf(20, "Available offline", None)]),
        submenu("Offline access", vec![leaf(30, "Available offline", None)]),
        submenu("Another extension", vec![leaf(40, "Online only", None)]),
    ];
    assert!(google_command(&items, CloudAvailability::Offline).is_none());
    assert!(google_command(&items, CloudAvailability::OnlineOnly).is_none());
}
