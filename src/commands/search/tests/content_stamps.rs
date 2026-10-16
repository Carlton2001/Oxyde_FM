use super::*;

#[test]
fn quick_and_advanced_results_carry_the_listing_revision() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("image.png");
    std::fs::write(&path, b"fixture").unwrap();
    let expected = crate::models::get_file_entry_from_path(&path)
        .unwrap()
        .content_stamp
        .unwrap();
    for advanced in [false, true] {
        let token = Arc::new(AtomicBool::new(false));
        let root = temp.path().to_string_lossy().into_owned();
        let receiver = if advanced {
            advanced_search(
                root,
                AdvancedSearchOptions {
                    query: "image".into(),
                    ..Default::default()
                },
                10,
                true,
                true,
                token,
            )
        } else {
            quick_search(root, "image".into(), 10, true, true, token)
        };
        let mut files = Vec::new();
        loop {
            match receiver
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap()
            {
                QuickSearchEvent::Batch(batch) => files.extend(batch),
                QuickSearchEvent::Finished { limited } => {
                    assert!(!limited);
                    break;
                }
                _ => panic!("unexpected search event"),
            }
        }
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].content_stamp.as_ref(), Some(&expected));
    }
}
