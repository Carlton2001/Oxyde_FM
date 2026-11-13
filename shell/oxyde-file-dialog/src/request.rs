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
    /// What fewer dialogs ask: their own labels and options, folders added to the
    /// places (`true` when above the others), the key of the folder remembered for the
    /// application, the folder to open when none is remembered, and the controls Oxyde
    /// shows besides choices.
    pub(crate) name_label: Option<String>,
    pub(crate) cancel_label: Option<String>,
    pub(crate) show_hidden: bool,
    pub(crate) no_overwrite_prompt: bool,
    pub(crate) strict_types: bool,
    pub(crate) no_read_only: bool,
    pub(crate) needs_interaction: bool,
    pub(crate) hide_pinned: bool,
    pub(crate) places: Vec<(bool, String)>,
    pub(crate) client: Option<String>,
    pub(crate) default_folder: Option<String>,
    pub(crate) fields: Vec<AskedField>,
    pub(crate) notes: Vec<String>,
    /// The button's menu, an open drop-down: its items, the one the button stands for.
    pub(crate) accept_menu: Option<AskedChoice>,
    /// The controls shown but not to be changed.
    pub(crate) disabled: Vec<u32>,
}

/// An edit box of the application's, as a text field.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct AskedField {
    pub(crate) id: u32,
    pub(crate) label: String,
    pub(crate) text: String,
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
    let labels = [("name-label", &asked.name_label), ("cancel-label", &asked.cancel_label)];
    for (key, value) in labels {
        if let Some(value) = value.as_deref().filter(|value| !value.is_empty()) {
            line(key, &plain(value));
        }
    }
    let options = [
        ("show-hidden", asked.show_hidden), ("no-overwrite-prompt", asked.no_overwrite_prompt), ("strict-types", asked.strict_types),
        ("no-read-only", asked.no_read_only), ("needs-interaction", asked.needs_interaction), ("hide-pinned", asked.hide_pinned),
    ];
    for (key, _) in options.iter().filter(|(_, set)| *set) {
        line(key, "1");
    }
    for (top, path) in &asked.places {
        line("place", &format!("{}\t{}", if *top { "top" } else { "bottom" }, plain(path)));
    }
    let folders = [("client", &asked.client), ("default-folder", &asked.default_folder)];
    for (key, value) in folders {
        if let Some(value) = value.as_deref().filter(|value| !value.is_empty()) {
            line(key, &plain(value));
        }
    }
    for field in &asked.fields {
        line("edit", &format!("{}\t{}\t{}", field.id, plain(&field.text), plain(&field.label)));
    }
    for note in &asked.notes {
        line("note", &plain(note));
    }
    if let Some(menu) = &asked.accept_menu {
        line("accept-menu", &format!("{}\t{}", menu.id, menu.initial));
        for (item, label) in &menu.options {
            line("option", &format!("{}\t{item}\t{}", menu.id, plain(label)));
        }
    }
    for id in &asked.disabled {
        line("disabled", &id.to_string());
    }
    text
}

/// What the user chose in Oxyde.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Answer {
    pub(crate) paths: Vec<String>,
    /// The filter shown when the user chose, counted from 1.
    pub(crate) filter: Option<u32>,
    /// Each control's id and its answer: an item's id, `true` or `false`, or a text.
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
    fn what_fewer_dialogs_ask_follows_the_choices() {
        let asked = Asked {
            saving: true,
            name_label: Some("Name".into()),
            cancel_label: Some(String::new()),
            no_overwrite_prompt: true,
            needs_interaction: true,
            places: vec![(true, r"C:\Work".into()), (false, r"D:\Old".into())],
            client: Some("app.exe:{1}".into()),
            default_folder: Some(r"C:\Users".into()),
            fields: vec![AskedField { id: 4, label: "Author".into(), text: "a\tb".into() }],
            notes: vec!["Signed only".into()],
            accept_menu: Some(AskedChoice { id: 12, label: String::new(), options: vec![(1, "Open".into()), (2, "Open read-only".into())], initial: "1".into() }),
            disabled: vec![4],
            ..Default::default()
        };
        assert_eq!(text(&asked), "purpose\tsave\nname-label\tName\nno-overwrite-prompt\t1\nneeds-interaction\t1\n\
            place\ttop\tC:\\Work\nplace\tbottom\tD:\\Old\nclient\tapp.exe:{1}\ndefault-folder\tC:\\Users\n\
            edit\t4\ta b\tAuthor\nnote\tSigned only\naccept-menu\t12\t1\noption\t12\t1\tOpen\noption\t12\t2\tOpen read-only\ndisabled\t4\n");
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
