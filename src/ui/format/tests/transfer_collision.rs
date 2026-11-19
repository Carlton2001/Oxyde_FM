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

#[test]
fn refused_replacement_of_a_link_is_translated_in_every_language() {
    use crate::ui::i18n::{I18n, Message};
    let detail = format!(
        "Cannot copy \"/a\" to \"/b\": {}",
        oxyde_lib::models::error::COPY_TARGET_NOT_REPLACEABLE
    );
    for language in Language::ALL {
        let i18n = I18n::for_language(language);
        let translated = error_detail(&detail, &i18n);
        assert_eq!(translated, i18n.text(Message::TransferTargetNotReplaceable).to_string());
        assert!(!translated.contains("Copy target"));
    }
}

#[test]
fn a_stale_trash_selection_is_translated_in_every_language() {
    use crate::ui::i18n::{FluentArgs, I18n, Message};
    let detail = oxyde_lib::models::error::CommandError::TrashError(
        oxyde_lib::models::error::TrashSelectionUnavailable { count: 3 }.to_string(),
    )
    .to_string();
    for language in Language::ALL {
        let i18n = I18n::for_language(language);
        let mut args = FluentArgs::new();
        args.set("count", 3);
        assert_eq!(error_detail(&detail, &i18n), i18n.text_with_args(Message::TrashSelectionUnavailable, &args).to_string());
        assert!(!error_detail(&detail, &i18n).contains("Recycle Bin"));
    }
}
