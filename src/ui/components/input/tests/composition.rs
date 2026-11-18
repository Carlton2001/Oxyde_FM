use super::{Span, commit, compose, shift_marked, spans};

/// Runs input method updates the way Windows sends them: each composition update without
/// a range, then the result without a range.
fn type_with_ime(content: &str, selected: std::ops::Range<usize>, updates: &[&str], result: &str) -> (String, std::ops::Range<usize>) {
    let mut content = content.to_string();
    let mut marked = None;
    let mut selected = selected;
    for update in updates {
        selected = compose(&mut content, &mut marked, selected, None, update, None);
        assert_eq!(marked, Some(selected.end - update.len()..selected.end));
    }
    let selected = commit(&mut content, &mut marked, selected, None, result).unwrap();
    assert_eq!(marked, None);
    (content, selected)
}

#[test]
fn each_composition_update_replaces_the_previous_one_and_the_commit_replaces_the_last() {
    assert_eq!(type_with_ime("ab", 2..2, &["k", "か", "かn", "かな"], "仮名"), ("ab仮名".into(), 8..8));
    assert_eq!(type_with_ime("ab", 1..1, &["ㅎ", "하", "한"], "한"), ("a한b".into(), 4..4));
    assert_eq!(type_with_ime("abc", 0..3, &["n", "你"], "你好"), ("你好".into(), 6..6));
}

#[test]
fn a_cancelled_composition_restores_the_text_and_an_empty_result_alone_changes_nothing() {
    let mut content = "ab".to_string();
    let mut marked = None;
    let selected = compose(&mut content, &mut marked, 1..1, None, "か", None);
    assert_eq!(commit(&mut content, &mut marked, selected, None, ""), Some(1..1));
    assert_eq!((content.as_str(), marked.clone()), ("ab", None));
    assert_eq!(commit(&mut content, &mut marked, 0..2, None, ""), None);
    assert_eq!(content, "ab");
}

#[test]
fn the_caret_an_input_method_asks_for_counts_utf16_units_within_the_composition() {
    let mut content = "x".to_string();
    let mut marked = None;
    let selected = compose(&mut content, &mut marked, 1..1, None, "𠀋か", Some(2..2));
    assert_eq!(selected, 5..5);
    assert_eq!(compose(&mut content, &mut marked, selected, None, "𠀋か", Some(1..1)), 5..5);
    assert_eq!(compose(&mut content, &mut marked, 5..5, None, "𠀋か", Some(3..0)), 1..8);
    assert_eq!(compose(&mut content, &mut marked, 1..1, None, "か", Some(9..9)), 4..4);
}

#[test]
fn an_explicit_range_wins_over_the_composition() {
    let mut content = "abc".to_string();
    let mut marked = Some(1..2);
    assert_eq!(commit(&mut content, &mut marked, 3..3, Some(1..2), ""), Some(1..1));
    assert_eq!((content.as_str(), marked), ("ac", None));
}

#[test]
fn field_edits_move_the_composition_or_end_it_when_they_reach_into_it() {
    assert_eq!(shift_marked(&(4..7), &(0..2), 5), Some(7..10));
    assert_eq!(shift_marked(&(4..7), &(4..4), 2), Some(6..9));
    assert_eq!(shift_marked(&(4..7), &(7..9), 0), Some(4..7));
    assert_eq!(shift_marked(&(4..7), &(3..5), 0), None);
    assert_eq!(shift_marked(&(4..7), &(5..5), 1), None);
}

#[test]
fn the_line_is_cut_where_the_selection_or_the_composition_changes() {
    let span = |len, selected, composing| Span { len, selected, composing };
    assert_eq!(spans(6, &(6..6), None), [span(6, false, false)]);
    assert_eq!(spans(6, &(6..6), Some(&(2..6))), [span(2, false, false), span(4, false, true)]);
    assert_eq!(
        spans(6, &(3..5), Some(&(2..6))),
        [span(2, false, false), span(1, false, true), span(2, true, true), span(1, false, true)]
    );
    assert_eq!(spans(4, &(1..9), None), [span(1, false, false), span(3, true, false)]);
}
