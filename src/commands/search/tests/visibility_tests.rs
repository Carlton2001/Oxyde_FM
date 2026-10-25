use super::*;
use std::path::Path;

fn results(receiver: std::sync::mpsc::Receiver<QuickSearchEvent>) -> Vec<FileEntry> {
    let mut files = Vec::new();
    loop {
        match receiver
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("search completion")
        {
            QuickSearchEvent::Batch(batch) => files.extend(batch),
            QuickSearchEvent::Finished { limited } => {
                assert!(!limited);
                return files;
            }
            QuickSearchEvent::Failed(error) | QuickSearchEvent::ContentSkipped { detail: error, .. } => panic!("{error}"),
            QuickSearchEvent::ArchiveSkipped { .. } => panic!("unexpected archive warning"),
        }
    }
}

fn search(root: &Path, advanced: bool, show_hidden: bool, show_system: bool) -> Vec<FileEntry> {
    let root = root.to_string_lossy().into_owned();
    let token = Arc::new(AtomicBool::new(false));
    results(if advanced {
        advanced_search(
            root,
            AdvancedSearchOptions {
                query: "needle".into(),
                recursive: true,
                ..Default::default()
            },
            100,
            show_hidden,
            show_system,
            token,
        )
    } else {
        quick_search(root, "needle".into(), 100, show_hidden, show_system, token)
    })
}

#[test]
fn user_directories_with_system_names_are_searchable() {
    let temp = tempfile::tempdir().unwrap();
    for name in [
        "Windows",
        "ProgramData",
        "$Recycle.Bin",
        "System Volume Information",
        "WinSxS",
    ] {
        let dir = temp.path().join(name);
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(dir.join("needle.txt"), b"fixture").unwrap();
    }
    for advanced in [false, true] {
        assert_eq!(search(temp.path(), advanced, false, false).len(), 5);
    }
}

#[cfg(target_os = "windows")]
fn set_attributes(path: &Path, attributes: u32) {
    crate::platform::platform()
        .set_fixture_file_attributes(path, attributes)
        .unwrap();
}

#[cfg(target_os = "windows")]
#[test]
fn listing_quick_and_advanced_share_attribute_precedence_and_traversal() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let temp = tempfile::tempdir().unwrap();
    for (name, attrs) in [("plain", 0x80), ("hidden", 2), ("system", 4), ("both", 6)] {
        let file = temp.path().join(format!("needle_{name}.txt"));
        std::fs::write(&file, b"fixture").unwrap();
        set_attributes(&file, attrs);
        let dir = temp.path().join(format!("{name}_branch"));
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(dir.join("needle_child.txt"), b"fixture").unwrap();
        set_attributes(&dir, attrs);
    }
    for (hidden, system, expected) in [
        (false, false, 1),
        (true, false, 2),
        (false, true, 3),
        (true, true, 4),
    ] {
        let listing = runtime
            .block_on(crate::commands::io::list_dir(
                temp.path().to_string_lossy().into_owned(),
                Some(hidden),
                Some(system),
            ))
            .unwrap();
        let listed: std::collections::BTreeSet<_> = listing
            .iter()
            .filter(|f| !f.is_dir)
            .map(|f| f.path.clone())
            .collect();
        assert_eq!(listed.len(), expected);
        for advanced in [false, true] {
            let files = search(temp.path(), advanced, hidden, system);
            let direct: std::collections::BTreeSet<_> = files
                .iter()
                .filter(|f| Path::new(&f.path).parent() == Some(temp.path()))
                .map(|f| f.path.clone())
                .collect();
            assert_eq!(
                direct, listed,
                "advanced={advanced}, hidden={hidden}, system={system}"
            );
            assert_eq!(
                files.len(),
                expected * 2,
                "branch pruning must use attributes too"
            );
        }
    }
}
