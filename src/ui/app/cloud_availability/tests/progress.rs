use super::{
    check_confirmation, cloud_location_affected, Badge, CloudAvailability, CloudBadges,
    CloudStatus, Confirmation, FileEntry,
};

#[test]
fn an_offline_request_needs_the_confirmed_offline_status() {
    for status in [
        CloudStatus::OnlineOnly,
        CloudStatus::LocallyAvailable,
        CloudStatus::SyncPending,
        CloudStatus::Syncing,
    ] {
        assert!(!CloudAvailability::Offline.is_confirmed_by(status));
    }
    assert!(CloudAvailability::Offline.is_confirmed_by(CloudStatus::AlwaysAvailable));
    assert!(CloudAvailability::OnlineOnly.is_confirmed_by(CloudStatus::OnlineOnly));
    assert!(!CloudAvailability::OnlineOnly.is_confirmed_by(CloudStatus::LocallyAvailable));
}

#[test]
fn a_pending_badge_changes_the_display_without_touching_the_listed_file() {
    let mut badges = CloudBadges::default();
    let path = "cloud-file".to_owned();
    let file = FileEntry {
        path: path.clone(),
        cloud_status: Some(CloudStatus::LocallyAvailable),
        ..FileEntry::default()
    };
    assert_eq!(
        badges.start(vec![path.clone()], 1, CloudAvailability::OnlineOnly),
        vec![path.clone()]
    );
    assert!(badges
        .start(vec![path.clone()], 2, CloudAvailability::OnlineOnly)
        .is_empty());
    let displayed = badges.entry(&file);
    assert_eq!(displayed.cloud_status, Some(CloudStatus::Syncing));
    assert!(displayed.is_cloud_placeholder);
    assert_eq!(file.cloud_status, Some(CloudStatus::LocallyAvailable));
    badges.release(std::slice::from_ref(&path), 1);
    assert_eq!(badges.status(&file), file.cloud_status);
}

#[test]
fn an_old_refresh_cannot_remove_a_new_request() {
    let mut badges = CloudBadges::default();
    let path = "cloud-file".to_owned();
    let file = FileEntry {
        path: path.clone(),
        ..FileEntry::default()
    };
    badges.0.insert(
        path.clone(),
        Badge {
            request: 1,
            availability: CloudAvailability::Offline,
            status: CloudStatus::AlwaysAvailable,
        },
    );
    assert_eq!(
        badges.start(vec![path.clone()], 2, CloudAvailability::OnlineOnly),
        vec![path.clone()]
    );
    badges.release(std::slice::from_ref(&path), 1);
    assert_eq!(badges.status(&file), Some(CloudStatus::Syncing));
}

#[test]
fn a_fresh_listing_can_confirm_the_change_before_the_next_probe() {
    let mut badges = CloudBadges::default();
    let path = "cloud-file".to_owned();
    badges.start(vec![path.clone()], 1, CloudAvailability::Offline);
    let file = FileEntry {
        path,
        cloud_status: Some(CloudStatus::AlwaysAvailable),
        ..FileEntry::default()
    };
    assert_eq!(badges.status(&file), file.cloud_status);
    assert!(!badges.entry(&file).is_cloud_placeholder);
}

#[test]
fn a_folder_change_refreshes_its_parent_and_open_descendants() {
    let folder = "/cloud/papers";
    assert!(cloud_location_affected("/cloud", folder));
    assert!(cloud_location_affected(folder, folder));
    assert!(cloud_location_affected("/cloud/papers/letters", folder));
    assert!(!cloud_location_affected("/cloud/papers-old", folder));
    #[cfg(windows)]
    assert!(cloud_location_affected(
        r"C:\Cloud\PAPERS\Letters",
        "c:/cloud/papers"
    ));
}

#[test]
fn a_folder_keeps_syncing_until_its_contents_are_ready() {
    for (availability, status) in [
        (CloudAvailability::Offline, CloudStatus::AlwaysAvailable),
        (CloudAvailability::OnlineOnly, CloudStatus::OnlineOnly),
    ] {
        let mut badges = CloudBadges::default();
        let path = "cloud-folder".to_owned();
        badges.start(vec![path.clone()], 1, availability);
        let file = FileEntry {
            path,
            is_dir: true,
            cloud_status: Some(status),
            ..FileEntry::default()
        };
        assert_eq!(badges.status(&file), Some(CloudStatus::Syncing));
        assert_eq!(
            check_confirmation(availability, status, || Some(false)),
            Confirmation::Pending
        );
        assert_eq!(
            check_confirmation(availability, status, || None),
            Confirmation::Unknown
        );
        assert_eq!(
            check_confirmation(availability, status, || Some(true)),
            Confirmation::Ready(Some(status))
        );
    }
}
