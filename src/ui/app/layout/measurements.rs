use crate::ui::app::OxydeApp;
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::scale::logical;
use super::ThemeContext;
use gpui::*;

impl OxydeApp {
    pub(crate) fn measure_metadata_columns(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let i18n = cx.i18n();
        let language = i18n.language();
        let reference = crate::ui::scale::normalized_reference(self.config.ui_rem_size);
        let date_format = crate::ui::format::date_format(cx);
        // Every format has the same number of tabular digits. One sample suffices.
        let date_sample = crate::ui::format::format_date(1_760_000_000_000, date_format);
        let measure = |text: &str, bold: bool, tabular: bool| column_text_width(window, cx, text, bold, tabular);
        self.state.layout.visit_panes_mut(&mut |pane| {
            let tab = pane.active_tab_mut();
            let (type_filtered, size_filtered, date_filtered, permissions_filtered) = tab.column_filter_flags();
            let key = (reference, language, date_format, [type_filtered, date_filtered, size_filtered, permissions_filtered]);
            if tab.column_measurement_key == Some(key) { return; }
            let type_width = tab.column_type_labels.iter().map(|label| measure(&type_column_text(label, i18n), false, false)).fold(0.0_f32, f32::max);
            tab.type_col_width = crate::ui::style::metadata_column_width(type_width, measure(i18n.text(Message::ColumnType).as_ref(), true, false), key.3[0]);
            tab.date_col_width = crate::ui::style::metadata_column_width(measure(&date_sample, false, true), measure(i18n.text(Message::ColumnModified).as_ref(), true, false), key.3[1]);
            tab.deleted_col_width = crate::ui::style::metadata_column_width(measure(&date_sample, false, true), measure(i18n.text(Message::TrashDeletedAt).as_ref(), false, false), false);
            let size_width = tab.column_size_values.iter().map(|&(size, directory, calculated)| measure(&size_column_text(size, directory, calculated, i18n), false, true)).fold(0.0_f32, f32::max);
            tab.size_col_width = crate::ui::style::metadata_column_width(size_width, measure(i18n.text(Message::ColumnSize).as_ref(), true, false), key.3[2]);
            // Fixed width: four tabular digits, the most the column ever shows.
            tab.permissions_col_width = crate::ui::style::metadata_column_width(measure("0000", false, true), measure(i18n.text(Message::ColumnPermissions).as_ref(), true, false), key.3[3]);
            tab.column_measurement_key = Some(key);
        });
    }

}

/// The width of `text` in a details column, in logical pixels: the bold of the
/// headers when `bold`, the tabular figures of the value cells when `tabular`.
pub(in crate::ui::app) fn column_text_width(window: &Window, cx: &App, text: &str, bold: bool, tabular: bool) -> f32 {
    let mut font = window.text_style().font();
    font.family = "Inter".into();
    font.weight = if bold { FontWeight::BOLD } else { crate::ui::style::DEFAULT_FONT_WEIGHT };
    font.features = if tabular { crate::ui::style::TNUM.clone() } else { FontFeatures::default() };
    let line = window.text_system().shape_line(text.to_string().into(), crate::ui::scale::rem_pixels(rems(if bold { 0.8125 } else { 0.75 })), &[TextRun {
        len: text.len(), font, color: cx.theme().text, background_color: None, underline: None, strikethrough: None,
    }], None);
    line.width.as_f32() / logical(1.0)
}

/// What the type column shows for a type label.
pub(in crate::ui::app) fn type_column_text(label: &str, i18n: &crate::ui::i18n::I18n) -> String {
    let message = match label {
        "Dossier" => Some(Message::FileTypeFolder), "Fichier" => Some(Message::FileTypeFile),
        "Symlink" => Some(Message::FileTypeSymlink), "Junction" => Some(Message::FileTypeJunction),
        "BrokenLink" => Some(Message::FileTypeBrokenLink),
        "Réseau" => Some(Message::SidebarNetwork), _ => None,
    };
    message.map(|message| i18n.text(message).to_string()).unwrap_or_else(|| label.to_string())
}

/// What the size column shows for a measured size.
pub(in crate::ui::app) fn size_column_text(size: u64, directory: bool, calculated: bool, i18n: &crate::ui::i18n::I18n) -> String {
    if directory && calculated && size == 0 {
        i18n.text(Message::FolderSizeEmpty).to_string()
    } else {
        crate::ui::format::format_size(size, directory, calculated, i18n.language())
    }
}
