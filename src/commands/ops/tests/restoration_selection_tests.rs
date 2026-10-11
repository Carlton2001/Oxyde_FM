    use super::*;
    fn item(id: &str, original: &str) -> trash::TrashItem {
        let original = PathBuf::from(original);
        trash::TrashItem {
            id: id.into(),
            name: original.file_name().unwrap().to_owned(),
            original_parent: original.parent().unwrap().to_owned(),
            time_deleted: 0,
        }
    }
    #[test]
    fn restoration_selects_exact_ids_first_and_deduplicates_requests() {
        let first = item("C:/bin/$Rfirst", "C:/original/first");
        let other = item("C:/bin/$Rother", "C:/bin/$Rfirst");
        let chosen = select_restoration_tasks(
            &[other, first.clone()],
            vec!["C:/bin/$Rfirst".into(), "C:/original/first".into()],
        )
        .unwrap();
        assert_eq!(chosen, vec![first]);
    }
    #[test]
    fn restoration_refuses_ambiguous_or_missing_original_paths_before_effects() {
        use crate::models::error::{TrashRestoreFailure, TrashRestoreIssue};
        let items = [
            item("C:/bin/$Rfirst", "C:/original/first"),
            item("C:/bin/$Rsecond", "C:/original/first"),
        ];
        for (path, issue) in [
            ("C:/original/first", TrashRestoreIssue::Ambiguous),
            ("C:/missing", TrashRestoreIssue::Unavailable),
        ] {
            let error =
                select_restoration_tasks(&items, vec!["C:/bin/$Rfirst".into(), path.into()])
                    .unwrap_err();
            assert_eq!(
                TrashRestoreFailure::from_message(&error.to_string()),
                Some(issue)
            );
        }
        assert_eq!(
            select_restoration_tasks(&items, vec!["C:/bin/$Rsecond".into()]).unwrap(),
            vec![items[1].clone()]
        );
    }
    #[test]
    fn restoration_requires_the_complete_corrected_name_without_stem_guessing() {
        let selected = item("C:/bin/$Rshortcut.lnk", "C:/original/shortcut.lnk");
        assert!(
            select_restoration_tasks(&[selected.clone()], vec!["C:/original/shortcut".into()])
                .is_err()
        );
        assert_eq!(
            select_restoration_tasks(&[selected.clone()], vec!["C:/original/shortcut.lnk".into()])
                .unwrap(),
            vec![selected]
        );
    }
