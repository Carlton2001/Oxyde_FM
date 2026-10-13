use super::*;

pub(super) fn render(view: &OxydeApp, handle: Entity<OxydeApp>, window: &mut Window, cx: &mut Context<OxydeApp>) -> AnyElement {
    let dialog = view.network_dialog.as_ref().expect("network dialog");
    let theme = *cx.theme();
            let viewport_h = rem_pixels(rems(8.875)).as_f32();
            let content_h = rem_pixels(rems(1.5)).as_f32() * 26.0;
            let mut choices = div()
                .id("network-letter-options")
                .w((rem_pixels(rems(30.0))
                    .min(crate::ui::components::popup::available_size(window, cx).width - rem_pixels(rems(2.0)))
                    - rem_pixels(rems(8.375)))
                .max(rem_pixels(rems(4.0))))
                .bg(theme.menu_surface)
                .occlude()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
                .h(rems(9.0))
                .relative()
                .flex()
                .flex_col()
                .border(rems(0.0625))
                .border_color(theme.border)
                .rounded_md();
            let refresh = handle.clone();
            let mut rows = div()
                .id("network-letter-scroll")
                .size_full()
                .overflow_y_scroll()
                .track_scroll(&dialog.letter_scroll)
                .pr(ZED_TRACK_WIDTH)
                .on_scroll_wheel(move |_, _, cx| refresh.update(cx, |_, cx| cx.notify()));
            for (index, character) in ('A'..='Z').rev().enumerate() {
                let value = format!("{character}:");
                let used = drive_letter_used(&value, &dialog.drives);
                let description = dialog
                    .drives
                    .iter()
                    .find(|drive| drive.path.to_ascii_uppercase().starts_with(&value))
                    .map(|drive| {
                        drive.remote_path.clone().unwrap_or_else(|| {
                            if drive.label == "Local Disk" {
                                let mut args = FluentArgs::new();
                                args.set("drive", value.clone());
                                cx.i18n()
                                    .text_with_args(Message::LocalDrive, &args)
                                    .to_string()
                            } else {
                                drive.label.clone()
                            }
                        })
                    });
                let selected = dialog.letter.as_ref() == Some(&value);
                let h = handle.clone();
                let key_handle = handle.clone();
                let key_value = value.clone();
                let key_action = std::rc::Rc::new(move |_: &mut Window, cx: &mut App| { key_handle.update(cx, |view, cx| { if let Some(dialog) = &mut view.network_dialog { if !used && !dialog.busy { dialog.letter = Some(key_value.clone()); dialog.letter_open = false; cx.notify(); } } }); });
                rows = rows.child(
                    crate::ui::components::popup::keyboard::item(div()
                        .id(("network-letter-option", index))
                        .h(rems(1.5))
                        .flex_none()
                        .flex()
                        .items_center()
                        .gap(rems(0.5))
                        .px_3()
                        .text_color(if used {
                            theme.icon_disabled
                        } else {
                            theme.text
                        })
                        .when(selected, |row| row.bg(theme.selected))
                        .when(!used, |row| {
                            row.hover(|style| style.bg(theme.control_hover))
                        })
                        .on_click(move |_, _, cx| {
                            h.update(cx, |view, cx| {
                                if let Some(dialog) = &mut view.network_dialog {
                                    if !used && !dialog.busy {
                                        dialog.letter = Some(value.clone());
                                        dialog.letter_open = false;
                                        cx.notify();
                                    }
                                }
                            })
                        })
                        .child(format!("{character}:"))
                        .when_some(description, |row, description| {
                            row.child(
                                div()
                                    .flex_1()
                                    .text_align(TextAlign::Right)
                                    .text_size(rems(0.6875))
                                    .text_ellipsis()
                                    .child(description),
                            )
                        }), key_action, None, Some((dialog.letter_scroll.clone(), index))),
                );
            }
            choices = choices.child(rows).child(render_scrollbar_with_pointer(
                theme,
                handle.clone(),
                ScrollbarAxis::Vertical,
                viewport_h,
                content_h,
                dialog.letter_scroll.offset().y.as_f32().abs(),
                dialog.letter_scroll_drag.is_some(),
                "network-letters".into(),
                None,
                |view, track, viewport, content, position, cx| {
                    if let Some(dialog) = &mut view.network_dialog {
                        dialog.letter_scroll_drag = Some((track, viewport, content));
                        update_letter_scroll(view, position, cx);
                    }
                },
            ));
            let mouse = handle.clone();
            choices = choices.child(
                canvas(
                    |_, _, _| (),
                    move |_, _, window, _| {
                        let mouse_move = mouse.clone();
                        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                            if phase == DispatchPhase::Capture {
                                mouse_move.update(cx, |view, cx| {
                                    if view
                                        .network_dialog
                                        .as_ref()
                                        .is_some_and(|dialog| dialog.letter_scroll_drag.is_some())
                                    {
                                        if event.dragging() {
                                            update_letter_scroll(view, crate::ui::components::popup::owner_point(window, cx, event.position), cx);
                                        } else if let Some(dialog) = &mut view.network_dialog {
                                            dialog.letter_scroll_drag = None;
                                            cx.notify();
                                        }
                                    }
                                });
                            }
                        });
                        let mouse_up = mouse.clone();
                        window.on_mouse_event(move |event: &MouseUpEvent, phase, _window, cx| {
                            if phase == DispatchPhase::Capture && event.button == MouseButton::Left
                            {
                                mouse_up.update(cx, |view, cx| {
                                    if let Some(dialog) = &mut view.network_dialog {
                                        if dialog.letter_scroll_drag.take().is_some() {
                                            cx.notify();
                                        }
                                    }
                                });
                            }
                        });
                    },
                )
                .absolute()
                .size_0(),
            );
    choices.into_any_element()
}