use crate::ui::app::OxydeApp;
use crate::ui::components::tooltip::Tooltip;
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::{OxydeTheme, ThemeContext};
use gpui::prelude::*;
use gpui::*;

pub(super) fn sidebar_button(
    id: String,
    label: SharedString,
    icon: Option<&'static str>,
    text: Option<String>,
    theme: OxydeTheme,
    enabled: bool,
    action: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    sidebar_button_with_tooltip(id, Tooltip::text(label), icon, text, theme, enabled, action)
}

pub(super) fn sidebar_button_with_tooltip(
    id: String,
    tooltip: impl Fn(&mut Window, &mut App) -> AnyView + 'static,
    icon: Option<&'static str>,
    text: Option<String>,
    theme: OxydeTheme,
    enabled: bool,
    action: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    crate::ui::elements::button::toolbar_button_with_tooltip(
        id, tooltip, icon, text, theme, enabled, action,
    )
}

pub(super) fn sidebar_toolbar(
    view: &OxydeApp,
    handle: Entity<OxydeApp>,
    scrolled: bool,
    cx: &mut Context<OxydeApp>,
) -> Div {
    let theme = *cx.theme();
    let mut toolbar = div()
        .h(crate::ui::style::TAB_BAR_HEIGHT)
        .flex_none()
        .flex()
        .items_center()
        .px_2()
        .gap_1();
    if !view.sidebar_collapsed {
        let h = handle.clone();
        toolbar = toolbar.child(sidebar_button(
            "sidebar-collapse-all".into(),
            cx.i18n().text(Message::SidebarCollapseAll),
            Some("icons/chevrons-up.svg"),
            None,
            theme,
            view.state
                .drives
                .iter()
                .any(|drive| view.state.expanded_paths.contains(&drive.path)),
            move |_, _, cx| {
                h.update(cx, |this, cx| {
                    this.sidebar_follow_task = None;
                    this.state.expanded_paths.clear();
                    this.state.refresh_sidebar();
                    this.reconcile_watchers(cx);
                    cx.notify();
                });
            },
        ));
        let h = handle.clone();
        toolbar = toolbar
            .child(sidebar_button(
                "sidebar-top".into(),
                cx.i18n().text(Message::SidebarScrollTop),
                Some("icons/arrow-up-to-line.svg"),
                None,
                theme,
                scrolled,
                move |_, _, cx| {
                    h.update(cx, |this, cx| {
                        let mut scroll = this.sidebar_scroll.0.borrow_mut();
                        // Reset the actual offset, even when the first row is already visible.
                        scroll.deferred_scroll_to_item = None;
                        let offset = scroll.base_handle.offset();
                        scroll.base_handle.set_offset(point(offset.x, px(0.0)));
                        cx.notify();
                    });
                },
            ))
            .child(div().flex_1());
    }
    toolbar.child(sidebar_button(
        "sidebar-toggle".into(),
        cx.i18n().text(if view.sidebar_collapsed {
            Message::SidebarExpand
        } else {
            Message::SidebarCollapse
        }),
        Some(if view.sidebar_collapsed {
            "icons/panel-left.svg"
        } else {
            "icons/panel-left-close.svg"
        }),
        None,
        theme,
        true,
        move |_, _, cx| {
            handle.update(cx, |this, cx| this.toggle_sidebar_panel(cx));
        },
    ))
}
