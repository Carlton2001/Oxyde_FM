use super::*;

#[test]
fn destination_collision_is_translated_and_preserves_the_literal_path() {
    use crate::ui::i18n::{FluentArgs, I18n, Message};
    let path = "C:\\école\\日本語 { $path }.txt";
    let failure = oxyde_lib::models::error::TransferDestinationCollision { path: path.into() };
    for language in Language::ALL {
        let i18n = I18n::for_language(language);
        let translated = error_detail(&failure.to_string(), &i18n);
        let mut args = FluentArgs::new();
        args.set("path", path);
        assert_eq!(
            translated,
            i18n.text_with_args(Message::TransferDestinationCollision, &args)
        );
        assert!(translated.contains(path));
        assert!(!translated.contains("Transfer destination collision:"));
        assert!(!translated.starts_with('['));
    }
}
