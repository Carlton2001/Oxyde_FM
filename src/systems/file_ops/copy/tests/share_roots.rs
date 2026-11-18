use super::*;

// Copying into the root of a share: Windows names it `\\?\UNC\server\share`
// without a separator, and every planned child fell outside it.
#[test]
fn files_planned_at_a_share_root_stay_inside_it() {
    let share = Path::new(r"\\localhost\C$");
    if std::fs::metadata(share).is_err() {
        return;
    }
    let tree = crate::systems::destination_tree::DestinationTree::new(share).unwrap();
    let root = tree.path().to_path_buf();
    assert!(root.join("planned.txt").starts_with(&root));
    let directories = [root.join("planned folder")];
    let files = [root.join("planned.txt"), root.join("planned folder").join("inner.txt")];
    validate(&root, &directories, files.iter(), &mut || Ok(())).unwrap();
}
