//! Below the entries, everything the application asks in one place: a form of
//! labeled rows (the name to save under, the type filters, the choices' menus, the
//! application's text fields, then the checkboxes and its notes) on the inactive panels' background, above the secondary
//! windows' usual footer and its buttons. The form grows with what is asked, up to a
//! third of the window, past which it scrolls; with many menus, they sit side by side. Tab and Shift+Tab move the keyboard from the
//! entries through its controls in reading order and back, as GTK's file chooser
//! does; Enter or Space presses the control that has it, and Escape cancels.
use super::FileChooserWindow;
use crate::ui::elements::button::ButtonVariant;
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::{OxydeTheme, ThemeContext};
use gpui::prelude::*;
use gpui::*;

/// The footer's own keyboard stops, the name field and the choices holding theirs,
/// and its form's scrolling.
pub(super) struct Footer {
    pub(super) types: FocusHandle,
    cancel: FocusHandle,
    accept: FocusHandle,
    pub(super) accept_menu: FocusHandle,
    scroll: ScrollHandle,
}

impl Footer {
    pub(super) fn new(cx: &mut App) -> Self {
        Self { types: cx.focus_handle(), cancel: cx.focus_handle(), accept: cx.focus_handle(), accept_menu: cx.focus_handle(), scroll: ScrollHandle::new() }
    }
}

/// Past this many menus, they sit side by side where the width allows.
const DENSE_MENUS: usize = 3;
/// Between a label and its control, and between the form's rows.
const LABEL_GAP: Rems = Rems(0.75);
const ROW_GAP: Rems = Rems(0.5);

/// The width of the longest label, in rems, which the form's label column takes.
fn label_column(labels: &[SharedString], window: &Window, cx: &App) -> f32 {
    let mut font = window.text_style().font();
    font.family = "Inter".into();
    font.weight = crate::ui::style::DEFAULT_FONT_WEIGHT;
    let rem = crate::ui::scale::rem_pixels(rems(1.0)).as_f32();
    labels.iter().map(|label| {
        let line = window.text_system().shape_line(label.clone(), crate::ui::scale::rem_pixels(rems(0.8125)), &[TextRun {
            len: label.len(), font: font.clone(), color: cx.theme().text, background_color: None, underline: None, strikethrough: None,
        }], None);
        line.width.as_f32() / rem
    }).fold(0.0, f32::max).ceil()
}

/// A label, ended to its control, in the label column.
fn label(text: SharedString, width: f32) -> Div {
    div().flex_none().w(rems(width)).flex().justify_end().child(text)
}

impl FileChooserWindow {
    /// The keyboard's stops in order: the entries, then the footer's controls shown.
    fn tab_order(&self, cx: &App) -> Vec<FocusHandle> {
        let mut order = vec![self.focus.clone()];
        order.extend(self.name.input.as_ref().map(|input| input.read(cx).focus_handle.clone()));
        if self.shows_type_filter() {
            order.push(self.footer.types.clone());
        }
        order.extend(self.choice_menu_focus());
        order.extend(self.field_focus());
        order.extend(self.choice_checkbox_focus());
        order.push(self.footer.cancel.clone());
        if self.can_accept(cx) {
            order.push(self.footer.accept.clone());
            if self.extras.accept_menu.is_some() {
                order.push(self.footer.accept_menu.clone());
            }
        }
        order
    }

    /// The form's row holding `focus`, as `render_footer` lays the rows out.
    fn form_row(&self, focus: &FocusHandle, cx: &App) -> Option<usize> {
        let mut rows: Vec<Vec<FocusHandle>> = Vec::new();
        rows.extend(self.name.input.as_ref().map(|input| vec![input.read(cx).focus_handle.clone()]));
        let menus: Vec<FocusHandle> = self.shows_type_filter().then(|| self.footer.types.clone()).into_iter()
            .chain(self.choice_menu_focus()).collect();
        if menus.len() > DENSE_MENUS {
            rows.push(menus);
        } else {
            rows.extend(menus.into_iter().map(|menu| vec![menu]));
        }
        // Each field has its row, disabled ones too.
        rows.extend(self.field_rows());
        rows.push(self.choice_checkbox_focus().collect());
        rows.iter().position(|row| row.contains(focus))
    }

    /// Tab and Escape while a stop has the keyboard; a field typing a path or a filter,
    /// or a name edited in place, keeps them.
    pub(super) fn footer_key(&mut self, key: &Keystroke, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let m = key.modifiers;
        if m.control || m.alt || m.platform || m.function { return false; }
        let order = self.tab_order(cx);
        let Some(current) = order.iter().position(|focus| focus.is_focused(window)) else { return false };
        match key.key.as_str() {
            "tab" => {
                let count = order.len();
                let next = if m.shift { (current + count - 1) % count } else { (current + 1) % count };
                window.focus(&order[next], cx);
                if let Some(row) = self.form_row(&order[next], cx) {
                    self.footer.scroll.scroll_to_item(row);
                }
                cx.notify();
            }
            // The entries keep their own Escape, which also cancels.
            "escape" if current > 0 => self.finish(None, window, cx),
            _ => return false,
        }
        true
    }

    pub(super) fn render_footer(&self, theme: OxydeTheme, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let i18n = cx.i18n();
        let default_label = if self.saving() { Message::FileChooserSave }
            else if self.directory { Message::FileChooserSelect }
            else { Message::Open };
        let accept = self.accept_label.clone().unwrap_or_else(|| i18n.text(default_label));
        let enabled = self.can_accept(cx);
        let (cancelled, accepted) = (cx.entity().downgrade(), cx.entity().downgrade());
        let cancel_label = self.extras.cancel_label.clone().filter(|label| !label.is_empty()).map(SharedString::from)
            .unwrap_or_else(|| i18n.text(Message::Cancel));
        let cancel = crate::ui::components::dialog::button("file-chooser-cancel", cancel_label, ButtonVariant::Secondary)
            .focus(&self.footer.cancel)
            .on_press(move |window, cx| { let _ = cancelled.update(cx, |this, cx| this.finish(None, window, cx)); });
        let open = crate::ui::components::dialog::button("file-chooser-open", accept, ButtonVariant::Primary)
            .focus(&self.footer.accept)
            .enabled(enabled)
            .on_press(move |window, cx| { let _ = accepted.update(cx, |this, cx| this.accept(window, cx)); });
        let name_label = self.extras.name_label.clone().filter(|label| !label.is_empty()).map(SharedString::from)
            .unwrap_or_else(|| i18n.text(Message::FileChooserName));
        let type_label = i18n.text(Message::FileChooserType);
        let open = self.render_accept(open.into_any_element(), enabled, theme, window, cx);
        let name = self.render_name_field(theme).map(|field| (name_label, field));
        let menus: Vec<(SharedString, AnyElement)> = self.render_type_filter(window, cx)
            .map(|menu| (type_label, menu)).into_iter()
            .chain(self.render_choice_menus(window, cx)).collect();
        let fields = self.render_fields(cx);
        let checkboxes = self.render_choice_checkboxes(theme, window, cx);
        let notes = self.render_notes(theme);
        // The secondary windows' footer, on their background to the window's lower corners.
        let corners = crate::ui::components::client_decorations::inner_corners(window);
        let footer = crate::ui::components::tool_window::footer(cx)
            .bg(theme.active_background).rounded_bl(corners.bottom_left).rounded_br(corners.bottom_right)
            .child(cancel)
            .child(open);
        let zone = div().flex_none().flex().flex_col();
        if name.is_none() && menus.is_empty() && fields.is_empty() && checkboxes.is_empty() && notes.is_empty() {
            return zone.child(footer);
        }

        let labels: Vec<SharedString> = name.iter().map(|(label, _)| label.clone())
            .chain(menus.iter().map(|(label, _)| label.clone()))
            .chain(fields.iter().map(|(label, _)| label.clone())).collect();
        let column = label_column(&labels, window, cx);
        let dense = menus.len() > DENSE_MENUS;
        // The form's rows, which the keyboard scrolls into view (`form_row`).
        let mut rows: Vec<AnyElement> = Vec::new();
        if let Some((text, field)) = name {
            rows.push(div().flex().items_center().gap(LABEL_GAP).child(label(text, column)).child(field).into_any_element());
        }
        let pairs = menus.into_iter().map(|(text, menu)| div()
            .flex().items_center().gap(LABEL_GAP)
            .when(!dense, |pair| pair.w_full())
            .when(dense, |pair| pair.flex_none())
            .child(label(text, column))
            .child(menu)
            .into_any_element());
        if dense {
            // Many menus wrap into columns, each pair as wide as the label column and a menu.
            rows.push(div().flex().flex_wrap().gap_x(rems(1.5)).gap_y(ROW_GAP).children(pairs).into_any_element());
        } else {
            rows.extend(pairs);
        }
        rows.extend(fields.into_iter().map(|(text, field)| div()
            .flex().items_center().gap(LABEL_GAP).child(label(text, column)).child(field).into_any_element()));
        // Below the menus and fields, on the controls' edge.
        let aligned = |content: Div| div().flex().items_center().gap(LABEL_GAP)
            .when(column > 0.0, |row| row.child(div().flex_none().w(rems(column))))
            .child(content.flex_1().min_w_0())
            .into_any_element();
        if !checkboxes.is_empty() {
            rows.push(aligned(div().flex().flex_wrap().gap_x(rems(1.5)).gap_y(ROW_GAP).children(checkboxes)));
        }
        if !notes.is_empty() {
            rows.push(aligned(div().flex().flex_col().gap(rems(0.25)).children(notes)));
        }

        // Up to a third of the window, past which the form scrolls with the shared scrollbar.
        let limit = window.viewport_size().height.as_f32() / 3.0;
        let scroll = self.footer.scroll.clone();
        let viewport = scroll.bounds().size.height.as_f32();
        let content = viewport + scroll.max_offset().y.as_f32();
        let scrolled = div()
            .relative()
            .min_h_0()
            .child(div().id("file-chooser-form").max_h(px(limit)).overflow_y_scroll().track_scroll(&scroll)
                .flex().flex_col().gap(ROW_GAP).children(rows))
            .when(content > viewport + 0.5, |area| area.child(self.render_vertical_scrollbar(theme, super::scroll::FORM, scroll, viewport, content, cx)));
        let form = div()
            .border_t(rems(0.0625))
            .border_color(theme.border)
            .bg(theme.background)
            .px(rems(1.0))
            .py(rems(0.625))
            .child(scrolled);
        zone.child(form).child(footer)
    }
}
