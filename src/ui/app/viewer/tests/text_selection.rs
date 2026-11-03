use super::super::{line_starts, wrap, Rows};
use std::sync::Arc;

fn rows(text: &str, columns: usize) -> Rows {
    let lines = line_starts(text);
    let rows = wrap(text, &lines, columns);
    Rows {
        text: Arc::new(text.to_owned()),
        rows: Arc::new(rows),
        lines: Arc::new(lines),
        highlights: Arc::new(Vec::new()),
        columns,
    }
}

#[test]
fn pointer_columns_map_to_the_nearest_character() {
    let rows = rows("ab\tc\nété\n", 80);
    assert_eq!(rows.offset_at(0, 0.4), 0);
    assert_eq!(rows.offset_at(0, 0.6), 1);
    // The tab spans columns 2 to 6.
    assert_eq!(rows.offset_at(0, 3.0), 2);
    assert_eq!(rows.offset_at(0, 5.0), 3);
    assert_eq!(rows.offset_at(0, 40.0), 4, "past the end stops before the line break");
    assert_eq!(rows.offset_at(1, 1.0), "ab\tc\n".len() + "é".len());
}

#[test]
fn selected_columns_follow_tabs_and_line_ends() {
    let rows = rows("ab\tc\nété\n", 80);
    let start = "ab\tc\n".len();
    assert_eq!(rows.selected_columns(0, &(1..start + 2)), Some((1, 7, true)));
    assert_eq!(rows.selected_columns(1, &(1..start + 2)), Some((0, 1, false)));
    assert_eq!(rows.selected_columns(1, &(0..2)), None);
}

#[test]
fn double_click_selects_the_word_under_the_pointer() {
    use super::word_at;
    let text = "let été_2 = x;";
    assert_eq!(&text[word_at(text, 4)], "été_2");
    assert_eq!(&text[word_at(text, 5)], "été_2", "inside a character");
    assert_eq!(&text[word_at(text, 3)], "let", "just past a word");
    assert_eq!(&text[word_at(text, text.find('=').unwrap())], "=");
    assert_eq!(word_at(text, text.len()), text.len()..text.len());
}
