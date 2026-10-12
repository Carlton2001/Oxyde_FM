#[test]
fn permanent_delete_keeps_del_without_shift() {
    for language in oxyde_lib::models::preferences::Language::ALL {
        let i18n = crate::ui::i18n::I18n::for_language(language);
        let label = i18n.text(crate::ui::i18n::Message::TrashDeletePermanent);
        assert_eq!(super::context_shortcut(&label, &i18n, false), Some("Del"));
        assert_eq!(super::context_shortcut(&label, &i18n, true), Some("Shift+Del"));
    }
}
