use crate::ui::app::OxydeApp;
use crate::ui::components::tooltip::{NativeTooltipExt, Tooltip};
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::OxydeTheme;
use gpui::prelude::*;
use gpui::*;

/// Occupies the existing left gutter without participating in list layout.
pub(super) fn render(
    view: &OxydeApp,
    pane_id: &str,
    handle: Entity<OxydeApp>,
    theme: OxydeTheme,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let all_selected = view.all_files_selected(pane_id);
    let pane_id = pane_id.to_string();
    div()
        .id(SharedString::from(format!("select-all-gutter-{pane_id}")))
        .absolute()
        .top_0()
        .left_0()
        .w(rems(0.5))
        .h(crate::ui::style::COLUMN_HEADER_HEIGHT)
        .hover(|style| style.bg(theme.control_hover))
        .when(view.context_menus.is_empty(), |button| {
            button.native_tooltip(Tooltip::with_shortcut(
                cx.i18n().text(if all_selected { Message::DeselectAll } else { Message::SelectAll }),
                Some(if all_selected { "Ctrl+Shift+A" } else { "Ctrl+A" }.into()),
            ))
        })
        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
            cx.stop_propagation();
            handle.update(cx, |view, cx| {
                view.finish_pointer_drag(cx);
                view.activate_pane(&pane_id, cx);
                view.focus_handle.focus(window, cx);
                if view.all_files_selected(&pane_id) {
                    view.deselect_all(&pane_id, cx);
                } else {
                    view.select_all(&pane_id, cx);
                }
            });
        })
}
