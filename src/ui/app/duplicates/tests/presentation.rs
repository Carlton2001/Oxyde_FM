use super::*;
use oxyde_lib::models::FileEntry;

fn group(sizes: &[u64]) -> DuplicateGroup {
    DuplicateGroup {
        size: sizes[0],
        files: sizes
            .iter()
            .map(|&size| FileEntry {
                size,
                ..Default::default()
            })
            .collect(),
    }
}

#[test]
fn a_large_group_resolves_visible_rows_without_a_row_vector_per_file() {
    let groups = [DuplicateGroup {
        size: 0,
        files: vec![FileEntry::default(); 100_000],
    }];
    let rows = Results::prepare(&groups, &[0], &HashSet::new());
    assert_eq!(rows.groups.len(), 1);
    assert_eq!(rows.len, 100_001);
    let visible: Vec<_> = (99_990..100_001).filter_map(|row| rows.row(row)).collect();
    assert_eq!(visible.len(), 11);
    assert_eq!(
        visible.last(),
        Some(&Row::File {
            group: 0,
            file: 99_999
        })
    );
    assert_eq!(rows.row(100_001), None);
}

#[test]
fn collapse_and_filter_preserve_original_group_and_file_identifiers() {
    let groups = [group(&[1, 100]), group(&[4, 4]), group(&[9, 10])];
    let rows = Results::prepare(&groups, &[2, 0], &HashSet::from([2]));
    assert_eq!(rows.len, 4);
    assert_eq!(rows.file_count, 6);
    assert!(rows.has_expanded);
    assert_eq!(
        rows.row(0),
        Some(Row::Header {
            group: 2,
            min_size: 9,
            max_size: 10,
            expanded: false
        })
    );
    assert_eq!(
        rows.row(1),
        Some(Row::Header {
            group: 0,
            min_size: 1,
            max_size: 100,
            expanded: true
        })
    );
    assert_eq!(rows.row(3), Some(Row::File { group: 0, file: 1 }));
    let collapsed = Results::prepare(&groups, &[0, 1, 2], &HashSet::from([0, 1, 2]));
    assert_eq!(collapsed.len, 3);
    assert!(!collapsed.has_expanded);
}

#[test]
fn common_and_mixed_sizes_are_prepared_independently_of_the_representative_size() {
    let groups = [group(&[100, 1]), group(&[4, 4])];
    let rows = Results::prepare(&groups, &[0, 1], &HashSet::new());
    assert_eq!(
        rows.row(0),
        Some(Row::Header {
            group: 0,
            min_size: 1,
            max_size: 100,
            expanded: true
        })
    );
    assert_eq!(
        rows.row(3),
        Some(Row::Header {
            group: 1,
            min_size: 4,
            max_size: 4,
            expanded: true
        })
    );
    assert_eq!(Results::default().row(0), None);
}
