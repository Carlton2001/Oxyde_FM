mod advanced;
mod comparison;
mod controls;
mod frame;
mod rules;

use advanced::render_advanced_body;
use comparison::render_comparison;
use controls::{edit, restore_focus, visible};
use frame::render_frame;

use crate::ui::app::{OxydeApp, PendingFileOperation};
use crate::ui::components::input::{InputAppearance, TextInput, TextInputOptions};
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::models::ConflictAction;
use std::collections::{HashMap, HashSet};

#[derive(Clone)]
pub(crate) struct ConflictDialogState {
    advanced: bool,
    width: f32,
    sidebar_width: f32,
    sidebar_resize: Option<(Point<Pixels>, f32)>,
    focus: Option<FocusHandle>,
    selected: usize,
    filter: usize,
    query: String,
    input: Option<Entity<TextInput>>,
    collapsed: HashSet<String>,
    pub(crate) resolutions: HashMap<String, ConflictAction>,
    automatic: HashSet<String>,
    opposite_initialized: bool,
}

impl Default for ConflictDialogState {
    fn default() -> Self {
        Self {
            advanced: false,
            width: 52.0,
            sidebar_width: 25.0,
            sidebar_resize: None,
            focus: None,
            selected: 0,
            filter: 0,
            query: String::new(),
            input: None,
            collapsed: HashSet::new(),
            resolutions: HashMap::new(),
            automatic: HashSet::new(),
            opposite_initialized: false,
        }
    }
}
impl ConflictDialogState {
    pub(crate) fn is_advanced(&self) -> bool { self.advanced }
    pub(crate) fn reset_focus(&mut self) { self.focus = None; }
}
pub(crate) fn render_conflict_dialog(
    pending_state: &mut PendingFileOperation,
    system: bool,
    simple_width: Option<Pixels>,
    window: &mut Window,
    handle: Entity<OxydeApp>,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let pending = pending_state;
    if pending.dialog.focus.is_none() {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        pending.dialog.focus = Some(focus);
    }
    if pending.dialog.advanced && pending.dialog.input.is_none() {
        let h = handle.clone();
        let escape = handle.clone();
        let input = cx.new(|cx| {
            TextInput::new(
                cx,
                "",
                String::new(),
                move |text, _, cx| {
                    edit(&h, cx, |p| {
                        p.dialog.query = text.to_string();
                        let rows = visible(p);
                        if !rows.contains(&p.dialog.selected) {
                            if let Some(&i) = rows.first() {
                                p.dialog.selected = i;
                            }
                        }
                    })
                },
                |_, _| {},
                move |window, cx| {
                    escape.update(cx, |a, cx| a.cancel_pending_file_operation(cx));
                    restore_focus(&escape, window, cx);
                },
                |_, _, _, _, _| {},
                TextInputOptions {
                    appearance: InputAppearance::Embedded,
                    clearable: true,
                    ..Default::default()
                },
                window,
            )
        });
        pending.dialog.input = Some(input.clone());
    }
    let advanced = pending.dialog.advanced;
    let max_width = (crate::ui::components::client_decorations::content_bounds(window).size.width / window.rem_size()).max(2.0);
    pending.dialog.width = max_width;
    let width = pending.dialog.width.min(max_width);
    let sidebar_width = pending.dialog.sidebar_width.min((width - 20.0).max(10.0));
    let rows = visible(pending);
    let current = &pending.conflicts[pending.dialog.selected];
    let comparison = render_comparison(pending, system, &handle, cx);
    let body = if advanced {
        render_advanced_body(
            pending,
            system,
            sidebar_width,
            rows,
            comparison,
            &handle,
            window,
            cx,
        )
    } else {
        let mut body = div().flex().flex_col().gap(rems(0.75));
        let mut args = FluentArgs::new();
        args.set("name", current.name.clone());
        body = body
            .child(
                div()
                    .text_size(rems(0.9375))
                    .child(cx.i18n().text_with_args(Message::ConflictPrompt, &args)),
            )
            .child(comparison);
        body
    };
    render_frame(pending, body, width, simple_width, handle, window, cx)
}
