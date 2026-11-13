//! The « Open With » prompt: applications by fit, as GNOME's chooser lists them, and
//! « Always use for this file type ».
use crate::ui::app::open_with::OpenWithPrompt;
use crate::ui::app::OxydeApp;
use crate::ui::components::dialog;
use crate::ui::components::modal::Modal;
use crate::ui::elements::button::ButtonVariant;
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::models::{ApplicationChoice, ApplicationFit};

const REMEMBER: usize = 0;
const CANCEL: usize = 1;
const OPEN: usize = 2;

const SECTIONS: [(ApplicationFit, Message); 3] = [
    (ApplicationFit::Recommended, Message::OpenWithRecommended),
    (ApplicationFit::Related, Message::OpenWithRelated),
    (ApplicationFit::Other, Message::OpenWithOtherApplications),
];

/// The list child showing choice `index`, counting the section headings before it.
pub(crate) fn row_child(prompt: &OpenWithPrompt, index: usize) -> usize {
    let Some(choices) = &prompt.choices else { return index };
    let Some(fit) = choices.get(index).map(|choice| choice.fit) else { return index };
    let visible = prompt.visible();
    let position = visible.iter().position(|shown| *shown == index).unwrap_or(0);
    let headings = SECTIONS.iter()
        .filter(|(section, _)| *section <= fit && visible.iter().any(|shown| choices[*shown].fit == *section))
        .count();
    position + headings
}

fn row(choice: &ApplicationChoice, index: usize, selected: bool, handle: &Entity<OxydeApp>, cx: &App) -> AnyElement {
    let theme = *cx.theme();
    let icon = match &choice.icon {
        Some(path) => img(path.clone()).size(rems(1.5)).object_fit(ObjectFit::Contain).into_any_element(),
        None => svg().path("icons/app-window.svg").size(rems(1.25)).text_color(theme.text_muted).into_any_element(),
    };
    let handle = handle.clone();
    div()
        .id(("open-with-choice", index))
        .flex()
        .items_center()
        .gap(rems(0.625))
        .flex_none()
        .h(rems(2.5))
        .px(rems(0.625))
        .rounded_md()
        .when(selected, |row| row.bg(theme.selected))
        .when(!selected, |row| row.hover(|row| row.bg(theme.hover)))
        .child(div().flex_none().size(rems(1.5)).flex().items_center().justify_center().child(icon))
        .child(div().flex_1().min_w_0().truncate().child(choice.name.clone()))
        .when(choice.is_default, |row| row.child(
            div().flex_none().text_size(rems(0.75)).text_color(theme.text_muted)
                .child(cx.i18n().text(Message::OpenWithDefault)),
        ))
        .on_mouse_down(MouseButton::Left, move |event, _, cx| {
            handle.update(cx, |view, cx| {
                view.select_open_with(index, cx);
                if event.click_count >= 2 {
                    view.answer_open_with(true, cx);
                }
            });
        })
        .into_any_element()
}

pub(crate) fn render_open_with_dialog(
    view: &OxydeApp,
    handle: Entity<OxydeApp>,
    window: &mut Window,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let prompt = view.open_with.as_ref().expect("open with prompt");
    let theme = *cx.theme();
    let i18n = cx.i18n();
    let intro = match &prompt.paths[..] {
        [only] => {
            let mut args = FluentArgs::new();
            let path = std::path::Path::new(only);
            args.set("name", path.file_name().map_or_else(|| only.clone(), |name| name.to_string_lossy().into_owned()));
            i18n.text_with_args(Message::OpenWithPrompt, &args)
        }
        paths => {
            let mut args = FluentArgs::new();
            args.set("count", paths.len());
            i18n.text_with_args(Message::OpenWithPromptMany, &args)
        }
    };
    let mut list = div()
        .id("open-with-list")
        .flex()
        .flex_col()
        .h(rems(20.0))
        .p(rems(0.375))
        .gap(rems(0.125))
        .overflow_y_scroll()
        .track_scroll(&prompt.scroll)
        .border(rems(0.0625))
        .border_color(theme.border)
        .rounded_md()
        .bg(theme.background);
    match &prompt.choices {
        None => list = list.child(div().p(rems(0.5)).text_color(theme.text_muted).child(i18n.text(Message::OpenWithLoading))),
        Some(choices) if prompt.visible().is_empty() => {
            let _ = choices;
            list = list.child(div().p(rems(0.5)).text_color(theme.text_muted).child(i18n.text(Message::OpenWithNone)));
        }
        Some(choices) => {
            let visible = prompt.visible();
            for (fit, heading) in SECTIONS {
                let shown: Vec<_> = visible.iter().copied().filter(|index| choices[*index].fit == fit).collect();
                if shown.is_empty() {
                    continue;
                }
                list = list.child(
                    div().px(rems(0.5)).pt(rems(0.5)).pb(rems(0.25)).text_size(rems(0.75))
                        .text_color(theme.text_muted).child(i18n.text(heading)),
                );
                for index in shown {
                    list = list.child(row(&choices[index], index, prompt.selected == Some(index), &handle, cx));
                }
            }
        }
    }
    let toggle = handle.clone();
    // The default application is already used for this type: the switch shows it, fixed.
    let is_default = prompt.selected
        .and_then(|index| prompt.choices.as_ref()?.get(index))
        .is_some_and(|choice| choice.is_default);
    let remember = div()
        .id("open-with-remember")
        .track_focus(&prompt.controls[REMEMBER])
        .when(is_default, |row| row.opacity(0.5))
        .flex()
        .items_center()
        .gap(rems(0.5))
        .pt(rems(0.25))
        .child(div().flex_1().min_w_0().child(i18n.text(Message::OpenWithAlways)))
        .child(crate::ui::elements::focus_ring::focus_ring(
            crate::ui::elements::toggle::toggle("switch", prompt.remember || is_default, theme),
            crate::ui::elements::focus_ring::keyboard_focused(&prompt.controls[REMEMBER], window),
            false,
            theme,
        ))
        .when(!is_default, |row| row.on_click(move |_, _, cx| toggle.update(cx, |view, cx| view.toggle_open_with_remember(cx))));
    let body = div()
        .flex()
        .flex_col()
        .gap(rems(0.5))
        .child(div().pb(rems(0.25)).child(intro))
        .child(div().w_full().child(prompt.input.clone()))
        .child(list)
        .child(remember);
    let (cancel, open, close) = (handle.clone(), handle.clone(), handle.clone());
    let can_open = prompt.selected.is_some_and(|index| prompt.visible().contains(&index));
    let modal = Modal::new("open-with", i18n.text(Message::OpenWith), body)
        .title_icon("icons/app-window.svg")
        .on_close(move |_, cx| close.update(cx, |view, cx| view.answer_open_with(false, cx)))
        .action(
            dialog::button("open-with-cancel", i18n.text(Message::Cancel), ButtonVariant::Secondary)
                .focus(&prompt.controls[CANCEL])
                .on_press(move |_, cx| cancel.update(cx, |view, cx| view.answer_open_with(false, cx))),
        )
        .action(
            dialog::button("open-with-open", i18n.text(Message::Open), ButtonVariant::Primary)
                .focus(&prompt.controls[OPEN])
                .enabled(can_open)
                .on_press(move |_, cx| open.update(cx, |view, cx| view.answer_open_with(true, cx))),
        );
    let keyboard = handle;
    div()
        .absolute()
        .inset_0()
        .track_focus(&prompt.focus)
        // In the search field, the arrows arrive as its completion actions.
        .on_action({
            let handle = keyboard.clone();
            move |_: &crate::ui::components::input::CompletionUp, _, cx| handle.update(cx, |view, cx| view.move_open_with_selection(-1, cx))
        })
        .on_action({
            let handle = keyboard.clone();
            move |_: &crate::ui::components::input::CompletionDown, _, cx| handle.update(cx, |view, cx| view.move_open_with_selection(1, cx))
        })
        .on_key_down(move |event, window, cx| {
            let typing = keyboard.read(cx).open_with.as_ref()
                .is_some_and(|prompt| prompt.input.read(cx).focus_handle.is_focused(window));
            keyboard.update(cx, |view, cx| {
                let Some(prompt) = &view.open_with else { return };
                let on_switch = prompt.controls[REMEMBER].is_focused(window);
                match event.keystroke.key.as_str() {
                    "escape" => view.answer_open_with(false, cx),
                    "tab" => {
                        let mut order = vec![prompt.input.read(cx).focus_handle.clone()];
                        order.extend(prompt.controls.iter().cloned());
                        dialog::cycle(&order, event.keystroke.modifiers.shift, window, cx);
                    }
                    "space" if on_switch => {
                        let is_default = prompt.selected
                            .and_then(|index| prompt.choices.as_ref()?.get(index))
                            .is_some_and(|choice| choice.is_default);
                        if !is_default {
                            view.toggle_open_with_remember(cx);
                        }
                    }
                    "enter" => view.answer_open_with(true, cx),
                    "up" if !typing => view.move_open_with_selection(-1, cx),
                    "down" if !typing => view.move_open_with_selection(1, cx),
                    "pageup" => view.move_open_with_selection(-8, cx),
                    "pagedown" => view.move_open_with_selection(8, cx),
                    _ => return,
                }
                cx.stop_propagation();
            })
        })
        .child(modal)
}
