use super::*;

#[test]
fn operation_item_labels_select_by_numeric_count() {
    for (language, item, items, affected, affecteds) in [
        (Language::French, "élément", "éléments", "Élément concerné", "Éléments concernés"),
        (Language::English, "item", "items", "Item involved", "Items involved"),
        (Language::German, "Element", "Elemente", "Betroffenes Element", "Betroffene Elemente"),
        (Language::Spanish, "elemento", "elementos", "Elemento de la operación", "Elementos de la operación"),
        (Language::Italian, "elemento", "elementi", "Elemento interessato", "Elementi interessati"),
    ] {
        let i18n = I18n::for_language(language);
        for count in [0, 1, 2, 1000] {
            let number = crate::ui::format::format_number(count as u64, language);
            let mut args = FluentArgs::new();
            args.set("count", count);
            args.set("done", "1");
            args.set("total", number.clone());
            args.set("number", number.clone());
            for (message, expected) in [
                (Message::OpsItems, format!("1 / {number} {}", if count == 1 { item } else { items })),
                (Message::OpsAffectedItems, format!("{} ({number})", if count == 1 { affected } else { affecteds })),
            ] {
                assert_eq!(
                    i18n.text_with_args(message, &args).replace(['\u{2068}', '\u{2069}'], ""),
                    expected,
                    "{language:?}, count={count}",
                );
            }
        }
    }
}
