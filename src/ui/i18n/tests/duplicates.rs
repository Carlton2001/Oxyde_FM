use super::*;

#[test]
fn duplicate_size_range_formats_both_endpoints_in_all_languages() {
    let mut args = FluentArgs::new();
    args.set("min", "1");
    args.set("max", "100");
    for language in Language::ALL {
        assert_eq!(I18n::for_language(language).text_with_args(Message::DuplicateSizeRange, &args)
            .replace(['\u{2068}', '\u{2069}'], ""), "1 – 100");
    }
}
