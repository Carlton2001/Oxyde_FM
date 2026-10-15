use super::{controls::choice, SettingsPicker};
use crate::ui::app::OxydeApp;
use crate::ui::components::popup;
use crate::ui::elements::button::{button_style, ButtonVariant};
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::{prelude::*, *};
use std::{cell::Cell, rc::Rc};

const FORMATS: [(&str, &str); 4] = [("zip", "Zip"), ("7z", "7z"), ("tar", "Tar"), ("zst", "Zst")];

fn choices(view: &OxydeApp, handle: Entity<OxydeApp>, cx: &App) -> AnyElement {
    let theme = *cx.theme();
    div()
        .w(rems(8.0))
        .p(rems(0.25))
        .rounded_md()
        .bg(theme.menu_surface)
        .border(rems(0.0625))
        .border_color(theme.menu_border)
        .shadow_md()
        .occlude()
        .children(FORMATS.map(|(format, label)| {
            let handle = handle.clone();
            choice(
                format,
                label.into(),
                view.default_archive_format() == format,
                theme,
                move |cx| {
                    handle.update(cx, |view, cx| {
                        view.archive_format_bounds = None;
                        view.set_default_archive_format(format, cx);
                    });
                },
            )
        }))
        .into_any_element()
}

pub(super) fn render(
    view: &OxydeApp,
    handle: Entity<OxydeApp>,
    window: &mut Window,
    cx: &mut Context<OxydeApp>,
) -> AnyElement {
    let theme = *cx.theme();
    let bounds = Rc::new(Cell::new(None));
    let paint_bounds = bounds.clone();
    let paint_handle = handle.clone();
    let action_handle = handle.clone();
    let action = Rc::new(move |_: &mut Window, cx: &mut App| {
        action_handle.update(cx, |view, cx| {
            view.archive_format_bounds = if view.archive_format_bounds.is_some() {
                None
            } else {
                bounds.get()
            };
            cx.notify();
        });
    });
    let pointer = action.clone();
    let label = FORMATS
        .iter()
        .find(|(format, _)| *format == view.default_archive_format())
        .unwrap()
        .1;
    let control = button_style(
        div().id("default-archive-format"),
        theme,
        ButtonVariant::Secondary,
        true,
    )
    .relative()
    .w(rems(5.0))
    .h(rems(1.75))
    .px(rems(0.5))
    .flex()
    .items_center()
    .gap(rems(0.5))
    .rounded_md()
    .child(
        canvas(
            |bounds, _, _| bounds,
            move |_, bounds, window, cx| {
                let bounds = popup::owner_bounds(window, cx, bounds);
                paint_bounds.set(Some(bounds));
                paint_handle.update(cx, |view, cx| {
                    if view
                        .archive_format_bounds
                        .is_some_and(|previous| previous != bounds)
                    {
                        view.archive_format_bounds = Some(bounds);
                        cx.notify();
                    }
                });
            },
        )
        .absolute()
        .size_full(),
    )
    .child(div().flex_1().child(label))
    .child(
        svg()
            .path("icons/chevron-down.svg")
            .size(rems(0.875))
            .text_color(theme.text_muted),
    )
    .on_mouse_down(MouseButton::Left, move |_, window, cx| {
        cx.stop_propagation();
        pointer(window, cx);
    });
    let row = div()
        .flex()
        .items_center()
        .gap(rems(0.5))
        .px(rems(0.375))
        .py(rems(0.5))
        .child(
            div()
                .flex_1()
                .child(cx.i18n().text(Message::DefaultArchiveFormat)),
        )
        .child(popup::keyboard::item(control, action, None, None));
    let mut result = div().child(row);
    if view.settings_open && view.settings_picker == Some(SettingsPicker::Compression) {
        if let Some(bounds) = view.archive_format_bounds {
            #[cfg(target_os = "windows")]
            if popup::supported(window, cx) {
                popup::show(
                    "default-archive-formats",
                    popup::PopupAnchor::element(bounds),
                    handle,
                    window,
                    cx,
                    |view| {
                        view.settings_open
                            && view.settings_picker == Some(SettingsPicker::Compression)
                            && view.archive_format_bounds.is_some()
                    },
                    |view, _, cx| choices(view, cx.entity(), cx),
                    |view, _, cx| {
                        view.archive_format_bounds = None;
                        cx.notify();
                    },
                );
                return result.into_any_element();
            }
            let dismiss = handle.clone();
            result = result.child(deferred(
                div()
                    .absolute()
                    .inset_0()
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        cx.stop_propagation();
                        dismiss.update(cx, |view, cx| {
                            view.archive_format_bounds = None;
                            cx.notify();
                        });
                    })
                    .child(
                        anchored()
                            .position(bounds.bottom_left())
                            .snap_to_window()
                            .child(choices(view, handle, cx)),
                    ),
            ));
        }
    }
    result.into_any_element()
}
