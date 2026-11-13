use super::*;
use crate::systems::file_ops::FileOpType;

fn paths(names: &[&str]) -> Vec<PathBuf> {
    names.iter().map(PathBuf::from).collect()
}

#[test]
fn common_parent_needs_every_source_in_one_folder() {
    assert_eq!(common_parent(&paths(&["/a/x", "/a/y"])), Some(PathBuf::from("/a")));
    assert_eq!(common_parent(&paths(&["/a/x", "/b/y"])), None);
    assert_eq!(common_parent(&[]), None);
}

#[test]
fn first_sources_follow_natural_name_order() {
    let sources = paths(&["/a/fichier-10", "/a/Fichier-2", "/a/big", "/a/fichier-1", "/a/dossier"]);
    assert_eq!(
        first_by_name(&sources, 3),
        paths(&["/a/big", "/a/dossier", "/a/fichier-1"]),
    );
    assert_eq!(
        first_by_name(&sources, 9),
        paths(&["/a/big", "/a/dossier", "/a/fichier-1", "/a/Fichier-2", "/a/fichier-10"]),
    );
}

#[test]
fn pauses_do_not_count_as_working_time() {
    let mut operation = FileOperation::new(FileOpType::Copy, paths(&["/a/x"]), None, None);
    operation.started_at = Some(Instant::now() - Duration::from_secs(10));
    operation.paused_since = Some(Instant::now() - Duration::from_secs(4));
    let paused = operation.active_duration().unwrap();
    assert!(paused <= Duration::from_millis(6_100) && paused >= Duration::from_secs(6));
    operation.pause_clock(false);
    assert!(operation.paused_since.is_none());
    assert!(operation.paused_total >= Duration::from_secs(4));
    operation.pause_clock(true);
    assert!(operation.paused_since.is_some());
}
