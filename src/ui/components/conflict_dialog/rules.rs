use super::ConflictDialogState;
use oxyde_lib::models::{ConflictAction, ConflictEntry};
use std::collections::HashMap;

impl ConflictDialogState {
    pub(super) fn record_choice(
        &mut self,
        conflicts: &[ConflictEntry],
        index: usize,
        action: ConflictAction,
    ) {
        let first = self.advanced && !self.opposite_initialized;
        if first {
            self.opposite_initialized = true;
            let opposite = match action {
                ConflictAction::Skip => ConflictAction::Replace,
                ConflictAction::Replace => ConflictAction::Skip,
            };
            let remaining: Vec<_> = conflicts
                .iter()
                .enumerate()
                .filter(|(i, c)| *i != index && decision(self, &c.source.path).is_none())
                .map(|(_, c)| c.source.path.clone())
                .collect();
            for path in remaining {
                self.resolutions.insert(path.clone(), opposite);
                self.automatic.insert(path);
            }
        }
        let selected = &conflicts[index];
        self.resolutions
            .insert(selected.source.path.clone(), action);
        self.automatic.remove(&selected.source.path);
        if selected.source.is_dir && !first {
            // Update automatic children, preserving explicit exceptions.
            for c in conflicts {
                if self.automatic.contains(&c.source.path)
                    && ancestors(&c.source.path).any(|a| a == selected.source.path)
                {
                    self.resolutions.insert(c.source.path.clone(), action);
                }
            }
        }
    }
    pub(super) fn counts(&self, conflicts: &[ConflictEntry]) -> (usize, usize) {
        conflicts.iter().fold((0, 0), |(replace, skip), c| {
            match decision(self, &c.source.path) {
                Some(ConflictAction::Replace) => (replace + 1, skip),
                Some(ConflictAction::Skip) => (replace, skip + 1),
                None => (replace, skip),
            }
        })
    }
    /// The queue rechecks conflicts, so every known pair needs an explicit decision.
    /// Unanswered pairs are skipped; newly appearing conflicts still fail that check.
    pub(crate) fn completed_resolutions(
        &self,
        conflicts: &[ConflictEntry],
    ) -> HashMap<String, ConflictAction> {
        let mut resolutions = self.resolutions.clone();
        for conflict in conflicts {
            resolutions
                .entry(conflict.source.path.clone())
                .or_insert_with(|| {
                    decision(self, &conflict.source.path).unwrap_or(ConflictAction::Skip)
                });
        }
        resolutions
    }
}
pub(super) fn category(c: &ConflictEntry) -> usize {
    if c.source.modified == c.target.modified && c.source.size == c.target.size {
        1
    } else if c.source.modified > c.target.modified {
        2
    } else if c.source.modified < c.target.modified {
        3
    } else {
        0
    }
}
pub(super) fn parent(path: &str) -> &str {
    path.rfind(std::path::is_separator).map_or("", |i| &path[..=i])
}
pub(super) fn ancestors(path: &str) -> impl Iterator<Item = &str> {
    let mut remaining = path;
    std::iter::from_fn(move || {
        let i = remaining.rfind(std::path::is_separator)?;
        remaining = &remaining[..i];
        Some(remaining)
    })
}
pub(super) fn decision(state: &ConflictDialogState, path: &str) -> Option<ConflictAction> {
    state
        .resolutions
        .get(path)
        .copied()
        .or_else(|| ancestors(path).find_map(|p| state.resolutions.get(p).copied()))
}
pub(super) fn filtered_indices(
    conflicts: &[ConflictEntry],
    state: &ConflictDialogState,
) -> Vec<usize> {
    let query = state.query.to_lowercase();
    conflicts
        .iter()
        .enumerate()
        .filter(|(_, c)| {
            (state.filter == 0 || category(c) == state.filter)
                && c.name.to_lowercase().contains(&query)
                && !ancestors(&c.source.path).any(|a| state.collapsed.contains(a))
        })
        .map(|(i, _)| i)
        .collect()
}
#[cfg(test)]
mod tests {
    use super::{category, decision, filtered_indices, ConflictDialogState};
    use oxyde_lib::models::{ConflictAction, ConflictEntry, FileEntry};
    fn conflict(path: &str, source_time: u64, target_time: u64, size: u64) -> ConflictEntry {
        ConflictEntry {
            name: path.rsplit('/').next().unwrap().into(),
            source: FileEntry {
                path: path.into(),
                modified: source_time,
                size,
                ..Default::default()
            },
            target: FileEntry {
                modified: target_time,
                size,
                ..Default::default()
            },
        }
    }
    #[test]
    fn categories_compare_source_to_target_and_require_equal_sizes() {
        assert_eq!(category(&conflict("a", 2, 1, 10)), 2);
        assert_eq!(category(&conflict("a", 1, 2, 10)), 3);
        assert_eq!(category(&conflict("a", 1, 1, 10)), 1);
        let mut c = conflict("a", 1, 1, 10);
        c.target.size = 20;
        assert_eq!(category(&c), 0);
    }
    #[test]
    fn closest_parent_decision_is_inherited_and_explicit_choice_wins() {
        let sep = std::path::MAIN_SEPARATOR;
        let path = |parts: &[&str]| parts.join(&sep.to_string());
        let mut state = ConflictDialogState::default();
        state.resolutions.insert(path(&["", "folder"]), ConflictAction::Skip);
        assert_eq!(decision(&state, &path(&["", "folder", "sub", "file"])), Some(ConflictAction::Skip));
        assert_eq!(decision(&state, &path(&["", "folder-other", "file"])), None);
        state.resolutions.insert(path(&["", "folder", "sub"]), ConflictAction::Replace);
        assert_eq!(decision(&state, &path(&["", "folder", "sub", "file"])), Some(ConflictAction::Replace));
        state.resolutions.insert(path(&["", "folder", "sub", "file"]), ConflictAction::Skip);
        assert_eq!(decision(&state, &path(&["", "folder", "sub", "file"])), Some(ConflictAction::Skip));
    }
    #[cfg(not(target_os = "windows"))]
    #[test]
    fn a_backslash_is_part_of_a_linux_name() {
        let mut state = ConflictDialogState::default();
        state.resolutions.insert("/x/a".into(), ConflictAction::Skip);
        assert_eq!(decision(&state, "/x/a\\b"), None);
        assert_eq!(super::parent("/x/a\\b"), "/x/");
    }
    #[test]
    fn filtered_rows_keep_original_indices_and_collapsed_descendants_are_hidden() {
        let conflicts = vec![
            conflict("root/older", 1, 2, 10),
            conflict("root/newer", 2, 1, 10),
        ];
        let mut state = ConflictDialogState {
            filter: 2,
            ..ConflictDialogState::default()
        };
        assert_eq!(filtered_indices(&conflicts, &state), vec![1]);
        state.query = "NEWER".into();
        assert_eq!(filtered_indices(&conflicts, &state), vec![1]);
        state.collapsed.insert("root".into());
        assert!(filtered_indices(&conflicts, &state).is_empty());
    }
    #[test]
    fn applying_partial_decisions_skips_unanswered_pairs_without_losing_inheritance() {
        let conflicts = vec![
            conflict("root", 1, 2, 10),
            conflict("root/child", 1, 2, 10),
            conflict("other", 1, 2, 10),
            conflict("root/explicit", 1, 2, 10),
        ];
        let mut state = ConflictDialogState::default();
        state
            .resolutions
            .insert("root".into(), ConflictAction::Replace);
        state
            .resolutions
            .insert("root/explicit".into(), ConflictAction::Skip);
        let choices = state.completed_resolutions(&conflicts);
        assert_eq!(choices["root"], ConflictAction::Replace);
        assert_eq!(choices["root/child"], ConflictAction::Replace);
        assert_eq!(choices["root/explicit"], ConflictAction::Skip);
        assert_eq!(choices["other"], ConflictAction::Skip);
        assert!(!choices.contains_key("new-conflict"));
    }
    #[test]
    fn first_advanced_choice_assigns_the_opposite_to_every_other_conflict() {
        let conflicts = vec![
            conflict("root/a", 2, 1, 10),
            conflict("root/b", 1, 2, 10),
            conflict("root/c", 1, 1, 10),
        ];
        for (first, opposite) in [
            (ConflictAction::Skip, ConflictAction::Replace),
            (ConflictAction::Replace, ConflictAction::Skip),
        ] {
            let mut state = ConflictDialogState {
                advanced: true,
                filter: 2,
                query: "a".into(),
                ..Default::default()
            };
            state.record_choice(&conflicts, 0, first);
            assert_eq!(decision(&state, "root/a"), Some(first));
            assert_eq!(decision(&state, "root/b"), Some(opposite));
            assert_eq!(decision(&state, "root/c"), Some(opposite));
            assert!(!state.automatic.contains("root/a"));
            assert!(state.automatic.contains("root/b"));
            assert!(state.automatic.contains("root/c"));
            assert_eq!(
                state.counts(&conflicts),
                if first == ConflictAction::Skip {
                    (2, 1)
                } else {
                    (1, 2)
                }
            );
        }
    }
    #[test]
    fn later_choices_override_defaults_without_reinitializing_other_decisions() {
        let conflicts = vec![
            conflict("a", 1, 2, 10),
            conflict("b", 1, 2, 10),
            conflict("c", 1, 2, 10),
        ];
        let mut state = ConflictDialogState {
            advanced: true,
            ..Default::default()
        };
        state.record_choice(&conflicts, 0, ConflictAction::Skip);
        state.record_choice(&conflicts, 1, ConflictAction::Skip);
        state.record_choice(&conflicts, 0, ConflictAction::Replace);
        assert_eq!(state.counts(&conflicts), (2, 1));
        assert!(!state.automatic.contains("a"));
        assert!(!state.automatic.contains("b"));
        assert!(state.automatic.contains("c"));
        state.advanced = false;
        state.advanced = true;
        state.record_choice(&conflicts, 1, ConflictAction::Replace);
        assert_eq!(state.counts(&conflicts), (3, 0));
        assert!(state.automatic.contains("c"));
    }
    #[test]
    fn simple_choices_remain_sequential_and_existing_choices_are_preserved() {
        let conflicts = vec![
            conflict("a", 1, 2, 10),
            conflict("b", 1, 2, 10),
            conflict("c", 1, 2, 10),
        ];
        let mut state = ConflictDialogState::default();
        state.record_choice(&conflicts, 0, ConflictAction::Skip);
        assert!(decision(&state, "b").is_none());
        assert!(state.automatic.is_empty());
        state.advanced = true;
        state.record_choice(&conflicts, 1, ConflictAction::Skip);
        assert_eq!(decision(&state, "a"), Some(ConflictAction::Skip));
        assert_eq!(decision(&state, "c"), Some(ConflictAction::Replace));
        assert!(!state.automatic.contains("a"));
    }
    #[test]
    fn folder_choices_update_automatic_children_and_keep_explicit_exceptions() {
        let mut conflicts = vec![
            conflict("root", 1, 2, 10),
            conflict("root/child", 1, 2, 10),
            conflict("root/exception", 1, 2, 10),
        ];
        conflicts[0].source.is_dir = true;
        let mut state = ConflictDialogState {
            advanced: true,
            ..Default::default()
        };
        state.record_choice(&conflicts, 0, ConflictAction::Skip);
        assert_eq!(
            decision(&state, "root/child"),
            Some(ConflictAction::Replace)
        );
        state.record_choice(&conflicts, 2, ConflictAction::Skip);
        state.record_choice(&conflicts, 0, ConflictAction::Replace);
        assert_eq!(
            decision(&state, "root/child"),
            Some(ConflictAction::Replace)
        );
        assert_eq!(
            decision(&state, "root/exception"),
            Some(ConflictAction::Skip)
        );
        state.record_choice(&conflicts, 0, ConflictAction::Skip);
        assert_eq!(decision(&state, "root/child"), Some(ConflictAction::Skip));
        assert_eq!(
            decision(&state, "root/exception"),
            Some(ConflictAction::Skip)
        );
    }
}
