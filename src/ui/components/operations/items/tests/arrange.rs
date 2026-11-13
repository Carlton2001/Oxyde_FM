use super::{arrange, Column, Item};
use crate::ui::workspace::SortDirection;
use oxyde_lib::systems::file_ops::ItemOutcome;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn rows(paths: &[&str]) -> Vec<Item> {
    paths.iter().map(|path| Item::new(Path::new(path), None, false)).collect()
}

fn names(rows: &[Item], shown: &[usize]) -> Vec<String> {
    shown.iter().map(|&index| rows[index].name.clone()).collect()
}

#[::core::prelude::v1::test]
fn names_sort_naturally_and_reverse() {
    let rows = rows(&["/a/f10", "/a/f2", "/a/F1"]);
    let none = HashMap::new();
    let up = arrange(&rows, "", Column::Name, SortDirection::Ascending, &none);
    assert_eq!(names(&rows, &up), ["F1", "f2", "f10"]);
    let down = arrange(&rows, "", Column::Name, SortDirection::Descending, &none);
    assert_eq!(names(&rows, &down), ["f10", "f2", "F1"]);
}

#[::core::prelude::v1::test]
fn filter_matches_the_whole_path() {
    let rows = rows(&["/photos/a.jpg", "/music/b.mp3", "/photos/c.png"]);
    let shown = arrange(&rows, "photos", Column::Name, SortDirection::Ascending, &HashMap::new());
    assert_eq!(names(&rows, &shown), ["a.jpg", "c.png"]);
}

#[::core::prelude::v1::test]
fn state_puts_running_and_failed_items_first() {
    let rows = rows(&["/a/done", "/a/failed", "/a/running"]);
    let outcomes: HashMap<PathBuf, ItemOutcome> = [
        ("/a/done", ItemOutcome::Completed),
        ("/a/failed", ItemOutcome::Failed),
        ("/a/running", ItemOutcome::InProgress),
    ]
    .into_iter()
    .map(|(path, outcome)| (PathBuf::from(path), outcome))
    .collect();
    let shown = arrange(&rows, "", Column::State, SortDirection::Ascending, &outcomes);
    assert_eq!(names(&rows, &shown), ["running", "failed", "done"]);
}

#[::core::prelude::v1::test]
fn location_groups_by_folder_then_name() {
    let rows = rows(&["/b/x", "/a/y", "/a/x"]);
    let shown = arrange(&rows, "", Column::Location, SortDirection::Ascending, &HashMap::new());
    assert_eq!(
        shown.iter().map(|&index| rows[index].path.clone()).collect::<Vec<_>>(),
        [PathBuf::from("/a/x"), PathBuf::from("/a/y"), PathBuf::from("/b/x")],
    );
}

#[::core::prelude::v1::test]
fn a_shared_folder_is_left_out_of_the_filter() {
    let rows: Vec<Item> = ["/tmp-1000/a10", "/tmp-1000/b"].iter().map(|path| Item::new(Path::new(path), None, true)).collect();
    let shown = arrange(&rows, "10", Column::Name, SortDirection::Ascending, &HashMap::new());
    assert_eq!(names(&rows, &shown), ["a10"]);
}
