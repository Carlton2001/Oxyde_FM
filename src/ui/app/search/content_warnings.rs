//! Localized disclosure when an Office document could not be searched completely.
use crate::ui::i18n::{FluentArgs, I18n, Message};
use gpui::SharedString;

#[derive(Clone, Default)]
pub struct ContentSearchWarnings {
    pub(super) count: u64,
    first_path: Option<String>,
}

impl ContentSearchWarnings {
    pub(super) fn record(&mut self, path: String) {
        self.count = self.count.saturating_add(1);
        if self.first_path.is_none() {
            self.first_path = Some(path);
        }
    }
    pub(super) fn label(&self, i18n: &I18n) -> SharedString {
        let mut args = FluentArgs::new();
        args.set("count", self.count as f64);
        args.set(
            "number",
            crate::ui::format::format_number(self.count, i18n.language()),
        );
        i18n.text_with_args(Message::QuickSearchContentSkipped, &args)
    }
    pub(super) fn detail(&self, i18n: &I18n) -> String {
        let mut args = FluentArgs::new();
        args.set("path", self.first_path.as_deref().unwrap_or_default());
        i18n.text_with_args(Message::QuickSearchContentSkippedDetail, &args)
            .to_string()
    }
}

#[cfg(test)]
#[path = "content_warnings/tests/translations.rs"]
mod tests;
