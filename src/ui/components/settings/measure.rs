//! Widths of the panel and its submenus, from the translated labels and the controls
//! that follow them, so no label is cut in any language and opening a submenu never
//! resizes the panel. Each length mirrors the geometry of `controls.rs`.
use super::SettingsPicker;
use crate::ui::i18n::{I18n, Message};
use crate::ui::theme::OxydeTheme;
use gpui::*;

/// A row's margins (2 × 0.375 rem), its icon (0.875) and the gap after it (0.5).
const ROW_LEAD: f32 = 0.375 * 2.0 + 0.875 + 0.5;
/// The gap between the label and what ends the row.
const GAP: f32 = 0.5;
/// A trailing chevron or external link icon.
const TRAILING_ICON: f32 = 0.875;
/// `elements::toggle`: a 1 rem thumb and its 0.875 rem travel.
const TOGGLE: f32 = 1.875;
/// `elements::stepper` around its value: two 1.5 rem buttons, the segmented frame's
/// padding, gaps and outline (2 × (0.125 + 0.125 + 0.0625)).
const STEPPER_FRAME: f32 = 1.5 * 2.0 + 0.625;
/// The stepper's value: at least 3.25 rem, its text inside 2 × 0.375 rem margins.
const STEPPER_VALUE_MIN: f32 = 3.25;
const STEPPER_VALUE_MARGIN: f32 = 0.375 * 2.0;
/// The left column's end: its 0.5 rem margin and 1/16 rem rule.
const LEFT_COLUMN_END: f32 = 0.5 + 0.0625;
/// Narrowest columns, so short languages keep a balanced panel.
const LEFT_MIN: f32 = 16.5;
const RIGHT_MIN: f32 = 17.5;
/// Space between the columns.
pub(super) const COLUMN_GAP: f32 = 0.5;
/// Width a submenu adds around its longest choice: the entry's margins (2 × 0.5
/// rem), check column and gap (1.5 rem), the surface's padding and outline.
const CHOICE_CHROME: f32 = 2.5 + 0.625;
/// Submenus whose explanations wrap at a readable width.
const COMPRESSION_WIDTH: f32 = 22.0;
const INTEGRATION_WIDTH: f32 = 24.0;
const ABOUT_WIDTH: f32 = 17.0;
/// Settings text: 0.8125 rem.
const TEXT_SIZE: f32 = 0.8125;

/// What ends a row after its label.
#[derive(Clone)]
pub(super) enum Trailing {
    Nothing,
    Icon,
    Toggle,
    /// A stepper showing at most this value.
    Stepper(SharedString),
}

pub(super) struct Measure<'a> {
    window: &'a Window,
    i18n: &'a I18n,
    font: Font,
    theme: OxydeTheme,
}

impl<'a> Measure<'a> {
    pub(super) fn new(window: &'a Window, i18n: &'a I18n, theme: OxydeTheme) -> Self {
        let mut font = window.text_style().font();
        font.family = "Inter".into();
        font.weight = crate::ui::style::DEFAULT_FONT_WEIGHT;
        Self { window, i18n, font, theme }
    }

    /// Width of `text` in rems.
    fn text(&self, text: SharedString) -> f32 {
        let run = TextRun {
            len: text.len(), font: self.font.clone(), color: self.theme.text,
            background_color: None, underline: None, strikethrough: None,
        };
        let line = self.window.text_system()
            .shape_line(text, crate::ui::scale::rem_pixels(rems(TEXT_SIZE)), &[run], None);
        line.width.as_f32() / crate::ui::scale::rem_pixels(rems(1.0)).as_f32()
    }

    fn row(&self, label: Message, trailing: &Trailing) -> f32 {
        let end = match trailing {
            Trailing::Nothing => 0.0,
            Trailing::Icon => GAP + TRAILING_ICON,
            Trailing::Toggle => GAP + TOGGLE,
            Trailing::Stepper(value) => {
                GAP + STEPPER_FRAME + (self.text(value.clone()) + STEPPER_VALUE_MARGIN).max(STEPPER_VALUE_MIN)
            }
        };
        ROW_LEAD + self.text(self.i18n.text(label)) + end
    }

    fn column(&self, rows: &[(Message, Trailing)], min: f32) -> f32 {
        rows.iter().map(|(label, trailing)| self.row(*label, trailing)).fold(min, f32::max)
    }

    pub(super) fn left(&self, rows: &[(Message, Trailing)]) -> f32 {
        self.column(rows, LEFT_MIN - LEFT_COLUMN_END) + LEFT_COLUMN_END
    }

    pub(super) fn right(&self, rows: &[(Message, Trailing)]) -> f32 {
        self.column(rows, RIGHT_MIN)
    }

    /// A submenu's width, at least as wide as any list.
    pub(super) fn submenu(&self, kind: SettingsPicker) -> f32 {
        let choices = |labels: &mut dyn Iterator<Item = SharedString>| {
            labels.map(|label| self.text(label)).fold(0.0_f32, f32::max) + CHOICE_CHROME
        };
        let width = match kind {
            SettingsPicker::Language => choices(&mut super::submenu::LANGUAGES.iter().map(|(_, label)| SharedString::from(*label))),
            SettingsPicker::Date => choices(&mut super::submenu::DATE_FORMATS.iter().map(|(_, label)| self.i18n.text(*label))),
            SettingsPicker::Compression => COMPRESSION_WIDTH,
            SettingsPicker::Integration => INTEGRATION_WIDTH,
            SettingsPicker::About => ABOUT_WIDTH,
        };
        width.max(crate::ui::components::select_menu::LIST_MIN_WIDTH.0)
    }
}
