use super::*;

fn entry(index: usize) -> FileEntry {
    let name = format!("{}{}.{}", ["a", "B", "c", "a"][index % 4], index % 5, ["txt", "LOG", ""][index % 3]);
    FileEntry {
        path: format!("C:/{}/{name}", ["x", "y"][index % 2]),
        name,
        is_dir: index % 7 == 0,
        size: (index % 6) as u64,
        modified: (index % 4) as u64,
        ..Default::default()
    }
}

#[test]
fn merging_a_batch_orders_as_sorting_everything_does() {
    let columns = [SortColumn::Name, SortColumn::Location, SortColumn::Type, SortColumn::Size, SortColumn::Modified];
    for column in columns {
        for direction in [SortDirection::Ascending, SortDirection::Descending] {
            let mut sorted: Vec<_> = (0..60).map(entry).collect();
            sort_files(&mut sorted, column, direction);
            let additions: Vec<_> = (60..100).map(entry).collect();
            let mut expected = sorted.clone();
            expected.extend(additions.iter().cloned());
            sort_files(&mut expected, column, direction);
            let merged = merge_sorted(&sorted, additions, column, direction);
            let paths = |files: &[FileEntry]| files.iter().map(|file| file.path.clone()).collect::<Vec<_>>();
            assert_eq!(paths(&merged), paths(&expected), "{column:?} {direction:?}");
        }
    }
}
