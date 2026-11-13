//! The application's type filters, offered in a menu of the footer's form. The
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

    /// Whether the footer offers the type filters: not when the application gives
    /// none, or when it asks for folders, which filters leave alone.
    pub(super) fn shows_type_filter(&self) -> bool {
        !self.directory && self.type_filter.is_some_and(|filter| filter < self.filters.len())
    }

    /// The filter shown and the menu of the others.
    pub(super) fn render_type_filter(&self, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.shows_type_filter() { return None; }
        let label = self.filters.get(self.type_filter?)?.name.clone();
        let focus = self.footer.types.clone();
        Some(self.render_menu(FooterMenu::Types, label, &focus, window, cx))
    }
}
