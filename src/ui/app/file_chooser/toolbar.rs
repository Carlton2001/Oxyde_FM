//! The file chooser's toolbar: history and folder buttons, the folder's crumbs or the
//! path being typed, the name filter, and the hidden files and view switches.
use super::location::{self, CrumbKind};
use super::FileChooserWindow;
use crate::ui::components::input::{CompletionDown, CompletionUp};
use crate::ui::components::tooltip::{NativeTooltipExt, Tooltip};
use crate::ui::elements::button::{button_style, ButtonVariant};
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::OxydeTheme;
use gpui::prelude::*;
use gpui::*;

const BUTTON: Rems = rems(1.875);

fn icon_button(id: &'static str, icon: &'static str, label: SharedString, enabled: bool, active: bool, theme: OxydeTheme) -> Stateful<Div> {
    button_style(div().id(id), theme, ButtonVariant::Ghost, enabled)
        .size(BUTTON).flex_none().flex().items_center().justify_center().rounded(rems(0.375))
        .when(active, |button| button.bg(theme.selected))
        .native_tooltip(Tooltip::text(label))
        .child(svg().path(icon).size(rems(1.0)).text_color(if enabled { theme.text } else { theme.icon_disabled }))
}

impl FileChooserWindow {
    fn render_crumbs(&self, theme: OxydeTheme, cx: &mut Context<Self>) -> AnyElement {
        if let Some(input) = &self.location {
            return div().flex_1().min_w_0().h(BUTTON).px(rems(0.5)).flex().items_center()
                .rounded(rems(0.375)).border(rems(0.0625)).bg(theme.surface)
                .border_color(if self.location_missing { theme.error } else { theme.accent })
                .child(div().flex_1().min_w_0().child(input.clone()))
                .into_any_element();
        }
        let home = dirs::home_dir().map(|home| home.to_string_lossy().into_owned());
        let crumbs = location::crumbs(&self.folder, home.as_deref());
        let last = crumbs.len().saturating_sub(1);
        let mut row = div().id("file-chooser-crumbs").h_full().min_w_0().flex().items_center()
            .overflow_x_scroll().track_scroll(&self.crumbs);
        for (index, crumb) in crumbs.into_iter().enumerate() {
            let current = index == last;
            if index > 0 {
                row = row.child(svg().path("icons/chevron-right.svg").size(rems(0.75)).flex_none().text_color(theme.text_muted));
            }
            let (icon, label) = match crumb.kind {
                CrumbKind::Root => (Some("icons/hard-drive.svg"), None),
                CrumbKind::Home => (Some("icons/house.svg"), Some(cx.i18n().text(Message::SidebarHome).to_string())),
                CrumbKind::Folder => (None, Some(crumb.name)),
            };
            let path = crumb.path;
            row = row.child(
                div().id(("file-chooser-crumb", index)).flex_none().h(rems(1.5)).px(rems(0.375))
                    .flex().items_center().gap(rems(0.375)).rounded(rems(0.375))
                    .text_color(if current { theme.text } else { theme.text_muted })
                    .when(!current, |crumb| crumb.hover(|crumb| crumb.bg(theme.hover).text_color(theme.text)))
                    .when_some(icon, |crumb, icon| crumb.child(svg().path(icon).size(rems(0.9375)).flex_none().text_color(theme.text)))
                    .when_some(label, |crumb, label| crumb.child(div().whitespace_nowrap().child(label)))
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        if event.click_count >= 2 {
                            this.begin_location(None, window, cx);
                        } else if !current {
                            this.show_folder(path.clone(), cx);
                        }
                    })),
            );
        }
        // The blank space after the crumbs edits the path, as in the main window.
        div().id("file-chooser-path").flex_1().min_w_0().h(BUTTON).flex().items_center()
            .child(row)
            .child(div().flex_1().h_full())
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| this.begin_location(None, window, cx)))
            .into_any_element()
    }

    fn render_filter(&self, theme: OxydeTheme, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::ui::components::input::field_frame(&self.filter, window, cx)
            .w(rems(11.0)).h(BUTTON).flex_none()
            // Its arrows move the list's selection, so Enter chooses what they reach.
            .on_action(cx.listener(|this, _: &CompletionUp, _, cx| this.move_line(false, cx)))
            .on_action(cx.listener(|this, _: &CompletionDown, _, cx| this.move_line(true, cx)))
            .child(svg().path("icons/search.svg").size(rems(0.875)).ml(rems(0.5)).flex_none().text_color(theme.text_muted))
            .child(div().flex_1().min_w_0().h_full().child(self.filter.clone()))
    }

    pub(super) fn render_toolbar(&self, theme: OxydeTheme, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let i18n = cx.i18n();
        let label = |message| i18n.text(message);
        let labels = [Message::Back, Message::Forward, Message::Up, Message::Refresh, Message::SettingsHidden, Message::ViewDetails, Message::ViewGrid].map(label);
        let [back_label, forward_label, up_label, refresh_label, hidden_label, details_label, grid_label] = labels;
        let (back, forward, up) = (!self.back.is_empty(), !self.forward.is_empty(), self.parent().is_some());
        let grid = self.options.grid;
        let hidden_icon = if self.show_hidden { "icons/eye.svg" } else { "icons/eye-off.svg" };
        div().flex_none().h(rems(2.75)).px(rems(0.5)).flex().items_center().gap(rems(0.25))
            .border_b(rems(0.0625)).border_color(theme.border).bg(theme.header)
            .child(icon_button("file-chooser-back", "icons/arrow-left.svg", back_label, back, false, theme)
                .when(back, |button| button.on_click(cx.listener(|this, _, _, cx| this.go_back(cx)))))
            .child(icon_button("file-chooser-forward", "icons/arrow-right.svg", forward_label, forward, false, theme)
                .when(forward, |button| button.on_click(cx.listener(|this, _, _, cx| this.go_forward(cx)))))
            .child(icon_button("file-chooser-up", "icons/arrow-up.svg", up_label, up, false, theme)
                .when(up, |button| button.on_click(cx.listener(|this, _, _, cx| this.go_up(cx)))))
            .child(icon_button("file-chooser-refresh", "icons/refresh-cw.svg", refresh_label, true, false, theme)
                .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))))
            .child(div().w(rems(0.25)).flex_none())
            .child(self.render_crumbs(theme, cx))
            .child(div().w(rems(0.25)).flex_none())
            .child(self.render_filter(theme, window, cx))
            .child(icon_button("file-chooser-hidden", hidden_icon, hidden_label, true, self.show_hidden, theme)
                .on_click(cx.listener(|this, _, _, cx| this.toggle_hidden(cx))))
            .child(icon_button("file-chooser-details", "icons/list.svg", details_label, true, !grid, theme)
                .when(grid, |button| button.on_click(cx.listener(|this, _, _, cx| this.toggle_grid(cx)))))
            .child(icon_button("file-chooser-grid", "icons/layout-grid.svg", grid_label, true, grid, theme)
                .when(!grid, |button| button.on_click(cx.listener(|this, _, _, cx| this.toggle_grid(cx)))))
    }
}
