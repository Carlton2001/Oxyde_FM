//! The application's type filters, offered in a menu at the start of the footer. The
//! filter shown decides which files the list offers and goes back with the choice.
use super::menus::FooterMenu;
use super::FileChooserWindow;
use gpui::*;

impl FileChooserWindow {
    pub(super) fn set_type_filter(&mut self, position: usize, cx: &mut Context<Self>) {
        if self.type_filter != Some(position) && position < self.filters.len() {
            self.type_filter = Some(position);
            self.admission.types = self.filters.get(position).cloned();
            let keep = self.kept();
            self.arrange(keep, cx);
        }
    }

    /// The filter shown and the menu of the others; nothing when the application gives
    /// none, or when it asks for folders, which filters leave alone.
    pub(super) fn render_type_filter(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.directory { return None; }
        let label = self.filters.get(self.type_filter?)?.name.clone();
        Some(self.render_menu(FooterMenu::Types, label, cx))
    }
}
