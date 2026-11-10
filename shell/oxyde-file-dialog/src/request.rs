//! What the library writes to the launcher and reads back: text, a line for each
//! `key<TAB>value`, as `src/platform/windows_file_dialog.rs` describes it. Oxyde reads
//! the request; nothing here knows how the launcher reaches it.

/// What an application asked its dialog for.
#[derive(Default)]
pub(crate) struct Asked {
    pub(crate) saving: bool,
    pub(crate) folders: bool,
    pub(crate) multiple: bool,
    pub(crate) title: Option<String>,
    pub(crate) accept: Option<String>,
    pub(crate) name: Option<String>,
    pub(crate) folder: Option<String>,
    pub(crate) extension: Option<String>,
    /// The application's window, as a number.
    pub(crate) owner: isize,
    /// Each filter's name and patterns.
    pub(crate) filters: Vec<(String, String)>,
    /// The filter shown first, counted from 1.
    pub(crate) filter: u32,
    pub(crate) choices: Vec<AskedChoice>,
}

/// A control of the application's, as a question beside the files.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct AskedChoice {
    pub(crate) id: u32,
    pub(crate) label: String,
    /// Each item's id and label; none for a check button.
    pub(crate) options: Vec<(u32, String)>,
    /// The item selected, or `true` or `false` for a check button.
    pub(crate) initial: String,
}

/// A value on its line: nothing an application names holds a tab or a line break.
fn plain(value: &str) -> String {
    value.replace(['\t', '\r', '\n'], " ")
}

pub(crate) fn text(asked: &Asked) -> String {
    let mut text = String::new();
    let mut line = |key: &str, value: &str| {
        text.push_str(key);
        text.push('\t');
        text.push_str(value);
        text.push('\n');
    };
    line("purpose", if asked.saving { "save" } else if asked.folders { "folder" } else { "open" });
    if asked.multiple {
        line("multiple", "1");
    }
    let values = [("title", &asked.title), ("accept", &asked.accept), ("name", &asked.name), ("folder", &asked.folder), ("extension", &asked.extension)];
    for (key, value) in values {
        if let Some(value) = value.as_deref().filter(|value| !value.is_empty()) {
            line(key, &plain(value));
        }
    }
    if asked.owner != 0 {
        line("owner", &asked.owner.to_string());
    }
    for (name, patterns) in &asked.filters {
        line("filter", &format!("{}\t{}", plain(name), plain(patterns)));
    }
    if !asked.filters.is_empty() {
        line("filter-index", &asked.filter.to_string());
    }
    for choice in &asked.choices {
        line("choice", &format!("{}\t{}\t{}", choice.id, choice.initial, plain(&choice.label)));
        for (item, label) in &choice.options {
            line("option", &format!("{}\t{item}\t{}", choice.id, plain(label)));
        }
    }
    text
}

/// What the user chose in Oxyde.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Answer {
    pub(crate) paths: Vec<String>,
    /// The filter shown when the user chose, counted from 1.
    pub(crate) filter: Option<u32>,
    /// Each control's id and its answer: an item's id, or `true` or `false`.
    pub(crate) choices: Vec<(u32, String)>,
}

/// The launcher's answer: the choice, nothing when the user cancelled, or an error
/// when it is not an answer.
pub(crate) fn answer(text: &str) -> Result<Option<Answer>, ()> {
    let mut lines = text.lines();
    match lines.next() {
        Some("cancel") => return Ok(None),
        Some("ok") => {}
        _ => return Err(()),
    }
    let mut answer = Answer::default();
    for line in lines {
        match line.split_once('\t') {
            Some(("path", path)) if !path.is_empty() => answer.paths.push(path.to_owned()),
            Some(("filter", filter)) => answer.filter = filter.parse().ok().filter(|filter| *filter > 0),
            Some(("choice", choice)) => {
                if let Some((id, value)) = choice.split_once('\t').and_then(|(id, value)| Some((id.parse().ok()?, value))) {
                    answer.choices.push((id, value.to_owned()));
                }
            }
            _ => {}
        }
    }
    if answer.paths.is_empty() {
        return Err(());
    }
    Ok(Some(answer))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_names_only_what_was_asked() {
        let asked = Asked { title: Some("Open\ta\nfile".into()), owner: 42, filters: vec![("Text".into(), "*.txt;*.md".into())], filter: 1, ..Default::default() };
        assert_eq!(text(&asked), "purpose\topen\ntitle\tOpen a file\nowner\t42\nfilter\tText\t*.txt;*.md\nfilter-index\t1\n");
        let asked = Asked { saving: true, name: Some("a.txt".into()), extension: Some("txt".into()), ..Default::default() };
        assert_eq!(text(&asked), "purpose\tsave\nname\ta.txt\nextension\ttxt\n");
        let asked = Asked { folders: true, multiple: true, folder: Some(r"C:\Users".into()), ..Default::default() };
        assert_eq!(text(&asked), "purpose\tfolder\nmultiple\t1\nfolder\tC:\\Users\n");
        let choices = vec![
            AskedChoice { id: 7, label: "Encoding".into(), options: vec![(1, "UTF-8".into()), (2, "ANSI".into())], initial: "2".into() },
            AskedChoice { id: 9, label: "Read\tonly".into(), options: Vec::new(), initial: "false".into() },
        ];
        let asked = Asked { choices, ..Default::default() };
        assert_eq!(text(&asked), "purpose\topen\nchoice\t7\t2\tEncoding\noption\t7\t1\tUTF-8\noption\t7\t2\tANSI\nchoice\t9\tfalse\tRead only\n");
    }

    #[test]
    fn an_answer_is_a_choice_a_cancellation_or_nothing_usable() {
        assert_eq!(answer("cancel\n"), Ok(None));
        assert_eq!(answer("ok\nfilter\t2\npath\tC:\\a.txt\npath\tC:\\b.txt\n"), Ok(Some(Answer { paths: vec![r"C:\a.txt".into(), r"C:\b.txt".into()], filter: Some(2), choices: Vec::new() })));
        assert_eq!(answer("ok\npath\tC:\\a.txt\nchoice\t7\t1\nchoice\tx\t1\n").map(|answer| answer.map(|answer| answer.choices)), Ok(Some(vec![(7, "1".into())])));
        assert_eq!(answer("ok\npath\tC:\\a.txt\n"), Ok(Some(Answer { paths: vec![r"C:\a.txt".into()], filter: None, choices: Vec::new() })));
        assert_eq!(answer("ok\n"), Err(()));
        assert_eq!(answer(""), Err(()));
        assert_eq!(answer("shown\n"), Err(()));
    }
}
