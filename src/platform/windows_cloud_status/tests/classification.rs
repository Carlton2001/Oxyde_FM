use super::*;

#[test]
fn distinguishes_cloud_availability_from_unrelated_offline_attributes() {
    let ready = CF_PLACEHOLDER_STATE_PLACEHOLDER | CF_PLACEHOLDER_STATE_IN_SYNC;
    assert_eq!(
        classify_cloud_file(ready, 0),
        Some(CloudStatus::LocallyAvailable)
    );
    assert_eq!(
        classify_cloud_file(ready, FILE_ATTRIBUTE_PINNED.0),
        Some(CloudStatus::AlwaysAvailable)
    );
    for attribute in [
        FILE_ATTRIBUTE_OFFLINE,
        FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS,
        FILE_ATTRIBUTE_RECALL_ON_OPEN,
    ] {
        assert_eq!(
            classify_cloud_file(ready, attribute.0),
            Some(CloudStatus::OnlineOnly)
        );
        assert_eq!(
            classify_cloud_file(CF_PLACEHOLDER_STATE(0), attribute.0),
            None
        );
    }
    assert_eq!(
        classify_cloud_file(
            ready | CF_PLACEHOLDER_STATE_PARTIALLY_ON_DISK,
            FILE_ATTRIBUTE_PINNED.0
        ),
        Some(CloudStatus::OnlineOnly),
    );
}

#[test]
fn distinguishes_pending_from_an_invalid_cloud_state() {
    assert_eq!(
        classify_cloud_file(CF_PLACEHOLDER_STATE_PLACEHOLDER, 0),
        Some(CloudStatus::SyncPending)
    );
    assert_eq!(classify_cloud_file(CF_PLACEHOLDER_STATE_INVALID, 0), None);
}
