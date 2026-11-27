use super::*;

fn entry(status: CloudStatus) -> FileEntry {
    FileEntry {
        cloud_status: Some(status),
        ..FileEntry::default()
    }
}

#[::core::prelude::v1::test]
fn offers_the_opposite_availability_for_one_direction_of_selection() {
    assert_eq!(
        selection_action(&[entry(CloudStatus::OnlineOnly)]),
        Some(CloudAvailability::Offline),
    );
    assert_eq!(
        selection_action(&[
            entry(CloudStatus::LocallyAvailable),
            entry(CloudStatus::AlwaysAvailable)
        ]),
        Some(CloudAvailability::OnlineOnly),
    );
}

#[::core::prelude::v1::test]
fn offers_no_action_for_mixed_local_or_syncing_entries() {
    for other in [
        entry(CloudStatus::LocallyAvailable),
        entry(CloudStatus::Syncing),
        entry(CloudStatus::SyncPending),
        FileEntry::default(),
    ] {
        assert_eq!(
            selection_action(&[entry(CloudStatus::OnlineOnly), other]),
            None
        );
    }
    assert_eq!(selection_action(&[]), None);
}

#[::core::prelude::v1::test]
fn uses_the_offline_command_when_it_is_the_only_one_available_to_all_cached_items() {
    let paths = ["file".to_owned(), "folder".to_owned()];
    let available = |path: &str| {
        Some(if path == "file" {
            vec![CloudAvailability::Offline, CloudAvailability::OnlineOnly]
        } else {
            vec![CloudAvailability::Offline]
        })
    };
    assert_eq!(
        common_available_action(&paths, CloudAvailability::OnlineOnly, true, available),
        Some(CloudAvailability::Offline),
    );
    assert_eq!(
        common_available_action(&paths, CloudAvailability::OnlineOnly, false, available),
        None,
    );
}
