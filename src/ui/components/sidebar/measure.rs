//! Widths of sidebar rows, for the width that shows the widest one: the main
//! sidebar's and the file chooser's, whose rows copy it.
use gpui::*;
use std::collections::HashMap;

/// Side padding, icon and gap of a quick access row.
pub(crate) const PLACE_CHROME: Rems = rems(0.75 + 1.0 + 0.5 + 0.75);
/// A drive or volume row adds the chevron's slot.
pub(crate) const DEVICE_CHROME: Rems = rems(0.75 + 1.0 + 0.5 + 0.75 + 1.0 + 0.25);
/// The list's left margin, and its right one without a vertical scrollbar.
pub(crate) const LIST_MARGIN: Rems = rems(0.5);
/// The right margin beside a vertical scrollbar: its track and a clear margin.
pub(crate) const SCROLLBAR_MARGIN: Rems = rems(1.125);

/// Measures sidebar labels the way they truncate: the sum of their characters'
/// own widths, without kerning. A label that shapes narrower would still be cut.
pub(crate) struct LabelWidths<'a> {
    text_system: &'a WindowTextSystem,
    regular: FontId,
    semibold: FontId,
    font_size: Pixels,
    cache: HashMap<(bool, char), f32>,
}

impl<'a> LabelWidths<'a> {
    pub(crate) fn new(window: &'a Window) -> Self {
        // Outside the root's rendering the window's text style is not the root's:
        // name the family it sets.
        let mut font = window.text_style().font();
        font.family = "Inter".into();
        let text_system = window.text_system();
        Self {
            text_system,
            regular: text_system.resolve_font(&Font { weight: crate::ui::style::DEFAULT_FONT_WEIGHT, ..font.clone() }),
            semibold: text_system.resolve_font(&Font { weight: FontWeight::SEMIBOLD, ..font }),
            font_size: crate::ui::scale::rem_pixels(rems(0.8125)),
            cache: HashMap::new(),
        }
    }

    /// The label's width in display pixels, rounded up with a pixel to spare.
    pub(crate) fn width(&mut self, text: &str, bold: bool) -> f32 {
        let (text_system, font, size) = (self.text_system, if bold { self.semibold } else { self.regular }, self.font_size);
        let width: f32 = text.chars()
            .map(|c| *self.cache.entry((bold, c)).or_insert_with(|| text_system.layout_width(font, size, c).as_f32()))
            .sum();
        width.ceil() + 1.0
    }
}

/// The list's two side margins around rows, with a vertical scrollbar or without.
pub(crate) fn list_margins(scrollbar: bool) -> f32 {
    crate::ui::scale::rem_pixels(LIST_MARGIN + if scrollbar { SCROLLBAR_MARGIN } else { LIST_MARGIN }).as_f32()
}
