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
        let measure = |text: &str, bold: bool| {
            let mut font = window.text_style().font();
            font.family = "Inter".into();
            font.weight = if bold { FontWeight::BOLD } else { crate::ui::style::DEFAULT_FONT_WEIGHT };
            font.features = crate::ui::style::TNUM.clone();
            let line = window.text_system().shape_line(text.to_string().into(), crate::ui::scale::rem_pixels(rems(if bold { 0.8125 } else { 0.75 })), &[TextRun {
                len: text.len(), font, color: cx.theme().text, background_color: None, underline: None, strikethrough: None,
            }], None);
            line.width.as_f32() / logical(1.0)
        };
        self.state.layout.visit_panes_mut(&mut |pane| {
            let tab = pane.active_tab_mut();
            let (type_filtered, size_filtered, date_filtered) = tab.metadata_filter_flags();
            let key = (reference, language, date_format, type_filtered, date_filtered, size_filtered);
            if tab.column_measurement_key == Some(key) { return; }
            let type_width = tab.column_type_labels.iter().map(|label| {
                let message = match label.as_str() {
                    "Dossier" => Some(Message::FileTypeFolder), "Fichier" => Some(Message::FileTypeFile),
                    "Symlink" => Some(Message::FileTypeSymlink), "Junction" => Some(Message::FileTypeJunction),
                    "BrokenLink" => Some(Message::FileTypeBrokenLink),
                    "Réseau" => Some(Message::SidebarNetwork), _ => None,
                };
                let text = message.map(|message| i18n.text(message).to_string()).unwrap_or_else(|| label.clone());
                measure(&text, false)
            }).fold(0.0_f32, f32::max);
            tab.type_col_width = crate::ui::style::metadata_column_width(type_width, measure(i18n.text(Message::ColumnType).as_ref(), true), key.3);
            tab.date_col_width = crate::ui::style::metadata_column_width(measure(&date_sample, false), measure(i18n.text(Message::ColumnModified).as_ref(), true), key.4);
            tab.deleted_col_width = crate::ui::style::metadata_column_width(measure(&date_sample, false), measure(i18n.text(Message::TrashDeletedAt).as_ref(), false), false);
            let size_width = tab.column_size_values.iter().map(|&(size, directory, calculated)| {
                if directory && calculated && size == 0 { measure(i18n.text(Message::FolderSizeEmpty).as_ref(), false) } else { measure(&crate::ui::format::format_size(size, directory, calculated, language), false) }
            }).fold(0.0_f32, f32::max);
            tab.size_col_width = crate::ui::style::metadata_column_width(size_width, measure(i18n.text(Message::ColumnSize).as_ref(), true), key.5);
            tab.column_measurement_key = Some(key);
        });
    }

}
