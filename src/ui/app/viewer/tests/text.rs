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
