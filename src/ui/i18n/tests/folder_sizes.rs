use super::*;

#[test]
fn pending_size_notice_keeps_three_explicit_sentence_lines_in_every_language() {
    for language in Language::ALL {
        let i18n = I18n::for_language(language);
        let text = i18n.text(Message::FolderSizeProvisionalDescription);
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines.len(), 3, "{language:?}: {text}");
        assert!(lines
            .iter()
            .all(|line| !line.is_empty() && line.ends_with('.')));
    }
}
