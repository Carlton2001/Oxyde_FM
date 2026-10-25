//! Background-prepared row ranges; rendering resolves only the visible indices.
use oxyde_lib::commands::duplicates::DuplicateGroup;
use std::collections::HashSet;

#[derive(Default)]
pub(super) struct Results {
    groups: Vec<Group>,
    pub(super) len: usize,
    pub(super) file_count: usize,
    pub(super) has_expanded: bool,
}

struct Group {
    index: usize,
    start: usize,
    count: usize,
    min_size: u64,
    max_size: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Row {
    Header {
        group: usize,
        min_size: u64,
        max_size: u64,
        expanded: bool,
    },
    File {
        group: usize,
        file: usize,
    },
}

impl Results {
    pub(super) fn prepare(
        groups: &[DuplicateGroup],
        indices: &[usize],
        collapsed: &HashSet<usize>,
    ) -> Self {
        let mut result = Self {
            file_count: groups.iter().map(|group| group.files.len()).sum(),
            ..Self::default()
        };
        for &index in indices {
            let group = &groups[index];
            let expanded = !collapsed.contains(&index);
            let count = 1 + if expanded { group.files.len() } else { 0 };
            let (min_size, max_size) =
                group.files.iter().fold((u64::MAX, 0), |(min, max), file| {
                    (min.min(file.size), max.max(file.size))
                });
            result.groups.push(Group {
                index,
                start: result.len,
                count,
                min_size: if group.files.is_empty() { 0 } else { min_size },
                max_size,
            });
            result.len += count;
            result.has_expanded |= expanded;
        }
        result
    }

    pub(super) fn row(&self, index: usize) -> Option<Row> {
        if index >= self.len {
            return None;
        }
        let group = &self.groups[self.groups.partition_point(|group| group.start <= index) - 1];
        Some(if index == group.start {
            Row::Header {
                group: group.index,
                min_size: group.min_size,
                max_size: group.max_size,
                expanded: group.count > 1,
            }
        } else {
            Row::File {
                group: group.index,
                file: index - group.start - 1,
            }
        })
    }
}

#[cfg(test)]
#[path = "tests/presentation.rs"]
mod tests;
