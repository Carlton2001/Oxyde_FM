use super::*;
use crate::commands::search::ContentPattern;

#[test]
fn shared_metadata_predicate_preserves_name_date_bounds_and_directory_rules() {
    let mut params = SearchParams {
        pattern: SearchPattern::Regex(
            regex::RegexBuilder::new(r"^ete\d+\.txt$")
                .case_insensitive(true)
                .build()
                .unwrap(),
            true,
        ),
        min_size: None,
        max_size: None,
        min_date: Some(10),
        max_date: Some(20),
        content: None,
    };
    for is_dir in [false, true] {
        assert!(params.matches_metadata("Été12.txt", is_dir, 0, Some(10)));
        assert!(params.matches_metadata("Été12.txt", is_dir, 0, Some(20)));
        assert!(!params.matches_metadata("Été12.txt", is_dir, 0, Some(9)));
        assert!(!params.matches_metadata("Été12.txt", is_dir, 0, Some(21)));
        assert!(!params.matches_metadata("Été12.txt", is_dir, 0, None));
        assert!(!params.matches_metadata("other.txt", is_dir, 0, Some(15)));
    }
    params.min_date = None;
    params.max_date = None;
    assert!(params.matches_metadata("Été12.txt", true, 0, None));
    params.content = Some(ContentPattern::new("anything", true).unwrap());
    assert!(!params.matches_metadata("Été12.txt", true, 0, None));
    assert!(params.matches_metadata("Été12.txt", false, 0, None));
}

#[test]
fn limitation_markers_are_recognized_only_as_complete_reasons() {
    for reason in [
        ArchiveSearchLimitation::ContentNotSearched,
        ArchiveSearchLimitation::DatesUnavailable,
    ] {
        assert_eq!(
            ArchiveSearchLimitation::from_message(reason.message()),
            Some(reason)
        );
        assert_eq!(
            ArchiveSearchLimitation::from_message(&format!("Archive Error: {}", reason.message())),
            Some(reason)
        );
        assert_eq!(
            ArchiveSearchLimitation::from_message(&format!(
                "bad archive named {}.zip",
                reason.message()
            )),
            None
        );
    }
}
