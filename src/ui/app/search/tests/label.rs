use super::*;
use oxyde_lib::{commands::search::AdvancedSearchOptions, models::preferences::Language};

#[::core::prelude::v1::test]
fn a_content_only_advanced_search_is_titled_with_its_content() {
    let options = AdvancedSearchOptions { content: "needle".into(), ..Default::default() };
    let path = format!(
        "search://{}",
        serde_json::to_string(&("advanced", "/tmp/data", options)).unwrap()
    );
    for language in Language::ALL {
        let i18n = crate::ui::i18n::I18n::for_language(language);
        let label = search_label(&path, &i18n).unwrap();
        assert!(label.contains("needle") && label.contains("data"), "{label}");
    }
}
