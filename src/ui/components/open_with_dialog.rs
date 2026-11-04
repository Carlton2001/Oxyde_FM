//! The « Open With » prompt: applications by fit, as GNOME's chooser lists them, and
//! « Always use for this file type ».
use crate::ui::app::open_with::OpenWithPrompt;
use crate::ui::app::OxydeApp;
use crate::ui::components::modal::{action_button, Modal};
use crate::ui::elements::button::{button_style, ButtonVariant};
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::models::{ApplicationChoice, ApplicationFit};

const SECTIONS: [(ApplicationFit, Message); 3] = [
    (ApplicationFit::Recommended, Message::OpenWithRecommended),
    (ApplicationFit::Related, Message::OpenWithRelated),
    (ApplicationFit::Other, Message::OpenWithOtherApplications),
];

/// The list child showing choice `index`, counting the section headings before it.
pub(crate) fn row_child(prompt: &OpenWithPrompt, index: usize) -> usize {
    let Some(choices) = &prompt.choices else { return index };
    let Some(fit) = choices.get(index).map(|choice| choice.fit) else { return index };
    let headings = SECTIONS.iter()
        .filter(|(section, _)| *section <= fit && choices.iter().any(|choice| choice.fit == *section))
        .count();
    index + headings
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
        .h(rems(2.25))
        .px(rems(0.5))
        .rounded_md()
        .when(selected, |row| row.bg(theme.selected))
        .when(!selected, |row| row.hover(|row| row.bg(theme.hover)))
        .child(div().flex_none().size(rems(1.5)).flex().items_center().justify_center().child(icon))
        .child(div().flex_1().min_w_0().truncate().child(choice.name.clone()))
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
    _window: &mut Window,
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
        .h(rems(18.0))
        .p(rems(0.25))
        .overflow_y_scroll()
        .track_scroll(&prompt.scroll)
        .border(rems(0.0625))
        .border_color(theme.border)
        .rounded_md()
        .bg(theme.background);
    match &prompt.choices {
        None => list = list.child(div().p(rems(0.5)).text_color(theme.text_muted).child(i18n.text(Message::OpenWithLoading))),
        Some(choices) if choices.is_empty() => {
            list = list.child(div().p(rems(0.5)).text_color(theme.text_muted).child(i18n.text(Message::OpenWithNone)));
        }
        Some(choices) => {
            for (fit, heading) in SECTIONS {
                if !choices.iter().any(|choice| choice.fit == fit) {
                    continue;
                }
                list = list.child(
                    div().px(rems(0.5)).pt(rems(0.5)).pb(rems(0.25)).text_size(rems(0.75))
                        .text_color(theme.text_muted).child(i18n.text(heading)),
                );
                for (index, choice) in choices.iter().enumerate().filter(|(_, choice)| choice.fit == fit) {
                    list = list.child(row(choice, index, prompt.selected == Some(index), &handle, cx));
                }
            }
        }
    }
    let toggle = handle.clone();
    let remember = div()
        .id("open-with-remember")
        .flex()
        .items_center()
        .gap(rems(0.5))
        .pt(rems(0.25))
        .child(crate::ui::elements::checkbox::checkbox(theme, prompt.remember, false))
        .child(i18n.text(Message::OpenWithAlways))
        .on_click(move |_, _, cx| toggle.update(cx, |view, cx| view.toggle_open_with_remember(cx)));
    let body = div()
        .flex()
        .flex_col()
        .gap(rems(0.5))
        .text_size(rems(0.8125))
        .child(div().pb(rems(0.25)).child(intro))
        .child(list)
        .child(remember);
    let (cancel, open, close) = (handle.clone(), handle.clone(), handle.clone());
    let can_open = prompt.selected.is_some();
    let modal = Modal::new("open-with", i18n.text(Message::OpenWith), body)
        .title_icon("icons/app-window.svg")
        .standalone(false)
        .body_background(theme.active_background)
        .on_close(move |_, cx| close.update(cx, |view, cx| view.answer_open_with(false, cx)))
        .width(rems(30.0))
        .action(
            button_style(action_button("open-with-cancel"), theme, ButtonVariant::Secondary, true)
                .rounded_md()
                .border(rems(0.0625))
                .border_color(theme.border)
                .child(i18n.text(Message::Cancel))
                .on_click(move |_, _, cx| cancel.update(cx, |view, cx| view.answer_open_with(false, cx))),
        )
        .action(
            button_style(action_button("open-with-open"), theme, ButtonVariant::Primary, can_open)
                .rounded_md()
                .border(rems(0.0625))
                .border_color(theme.accent)
                .child(i18n.text(Message::Open))
                .on_click(move |_, _, cx| open.update(cx, |view, cx| view.answer_open_with(true, cx))),
        );
    let keyboard = handle;
    div()
        .absolute()
        .inset_0()
        .track_focus(&prompt.focus)
        .on_key_down(move |event, _, cx| {
            keyboard.update(cx, |view, cx| {
                match event.keystroke.key.as_str() {
                    "escape" => view.answer_open_with(false, cx),
                    "enter" => view.answer_open_with(true, cx),
                    "up" => view.move_open_with_selection(-1, cx),
                    "down" => view.move_open_with_selection(1, cx),
                    "pageup" => view.move_open_with_selection(-8, cx),
                    "pagedown" => view.move_open_with_selection(8, cx),
                    "home" => view.move_open_with_selection(isize::MIN, cx),
                    "end" => view.move_open_with_selection(isize::MAX, cx),
                    "space" => view.toggle_open_with_remember(cx),
                    _ => return,
                }
                cx.stop_propagation();
            })
        })
        .on_action(|_: &crate::ui::app::Ouvrir, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Supprimer, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Renommer, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Copier, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Couper, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Coller, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::ToutSelectionner, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Annuler, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Refaire, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Actualiser, _, cx| cx.stop_propagation())
        .child(modal)
}
