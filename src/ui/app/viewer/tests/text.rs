use super::{line_starts, wrap};

fn rows(text: &str, columns: usize) -> Vec<&str> {
    let lines = line_starts(text);
    let starts = wrap(text, &lines, columns);
    starts
        .iter()
        .enumerate()
        .map(|(index, start)| {
            let end = starts.get(index + 1).map_or(text.len(), |end| *end as usize);
            text[*start as usize..end].trim_end_matches(['\n', '\r'])
        })
        .collect()
}

#[test]
fn every_line_is_kept_and_words_wrap_at_spaces() {
    assert_eq!(rows("hello wide world\r\nnext", 11), ["hello wide ", "world", "next"]);
    assert_eq!(rows("a\n\nb\n", 10), ["a", "", "b"]);
    assert_eq!(rows("", 10), [""]);
}

#[test]
fn words_longer_than_a_row_break_inside_and_wide_characters_count_twice() {
    assert_eq!(rows("abcdefgh", 3), ["abc", "def", "gh"]);
    assert_eq!(rows("日本語の", 4), ["日本", "語の"]);
    // A tab is four columns and a break opportunity: the word moves down whole.
    assert_eq!(rows("\tab", 5), ["\t", "ab"]);
}

#[test]
fn line_numbers_skip_continuations_and_styling_expands_tabs() {
    use super::{Rows, syntax};
    use std::sync::Arc;
    let text = "\tété long\nnext";
    let lines = line_starts(text);
    let rows = Rows {
        text: Arc::new(text.into()),
        rows: Arc::new(wrap(text, &lines, 8)),
        lines: Arc::new(lines),
        highlights: Arc::new(vec![syntax::Span { range: 0..7, kind: 2 }]),
        columns: 8,
    };
    assert_eq!(rows.line_number(0), "1");
    assert_eq!(rows.line_number(1), "");
    assert_eq!(rows.line_number(2), "2");
    assert_eq!(rows.row(0).unwrap().as_ref(), "    été ");
    let highlights = rows.row_highlights(0, &crate::ui::theme::OxydeTheme::dark());
    assert_eq!(highlights.len(), 1);
    assert_eq!(highlights[0].0, 0..10);
    assert!(rows.row_highlights(1, &crate::ui::theme::OxydeTheme::dark()).is_empty());
}

#[test]
fn highlights_are_clipped_to_each_wrapped_utf8_row() {
    use super::{Rows, syntax};
    use std::sync::Arc;
    let text = "éété";
    let lines = line_starts(text);
    let rows = Rows {
        text: Arc::new(text.into()),
        rows: Arc::new(wrap(text, &lines, 2)),
        lines: Arc::new(lines),
        highlights: Arc::new(vec![syntax::Span { range: 0..text.len(), kind: 2 }]),
        columns: 2,
    };
    let theme = crate::ui::theme::OxydeTheme::light();
    assert_eq!(rows.row_highlights(0, &theme)[0].0, 0..4);
    assert_eq!(rows.row_highlights(1, &theme)[0].0, 0..3);
}

#[test]
fn wrapped_rows_preserve_indent_and_shift_syntax_ranges() {
    use super::{Rows, syntax};
    use std::sync::Arc;
    for (source, columns, indent) in [("    alpha beta gamma", 12, 4), ("\talpha beta gamma", 12, 4), ("        alpha beta", 6, 3)] {
        let lines = line_starts(source);
        let starts = wrap(source, &lines, columns);
        let rows = Rows {
            text: Arc::new(source.into()),
            rows: Arc::new(starts),
            lines: Arc::new(lines),
            highlights: Arc::new(vec![syntax::Span { range: 0..source.len(), kind: 2 }]),
            columns,
        };
        assert!(rows.rows.len() > 1);
        for index in 1..rows.rows.len() {
            let row = rows.row(index).unwrap();
            assert!(row.starts_with(&" ".repeat(indent)));
            assert!(row.chars().count() <= columns);
            assert_eq!(rows.line_number(index), "");
            assert_eq!(rows.row_highlights(index, &crate::ui::theme::OxydeTheme::dark())[0].0.start, indent);
        }
    }
}

#[test]
fn wrapping_never_loses_source_bytes_in_narrow_panes() {
    for source in ["    alpha beta gamma\n  next", "\t日本語の texte\r\n", "            ", "éé abc"] {
        for columns in 1..20 {
            let starts = wrap(source, &line_starts(source), columns);
            assert!(starts.windows(2).all(|pair| pair[0] < pair[1]));
            let restored: String = starts.iter().enumerate().map(|(index, &start)| {
                let end = starts.get(index + 1).map_or(source.len(), |end| *end as usize);
                &source[start as usize..end]
            }).collect();
            assert_eq!(restored, source);
        }
    }
}
