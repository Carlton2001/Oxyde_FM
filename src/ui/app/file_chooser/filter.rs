//! The name filter at the end of the navigation bar, a copy of the main window's quick
//! search control (`render_quick_search_control`): a separator, a field sliding out of
//! the magnifier on hover, click or Ctrl+F, collapsing on a click elsewhere. Here it
//! filters the folder's names as typed, and Enter chooses the selection.
use super::FileChooserWindow;
use crate::ui::components::input::{CompletionDown, CompletionUp, InputAppearance, TextInput, TextInputOptions};
use crate::ui::components::tooltip::{NativeTooltipExt, Tooltip};
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::OxydeTheme;
use gpui::prelude::*;
use gpui::*;

const SLIDE: f32 = 0.25;

pub(super) struct Filter {
    pub(super) input: Option<Entity<TextInput>>,
    pub(super) text: String,
    expanded: bool,
    hover_suppressed: bool,
    from: f32,
    started: std::time::Instant,
    revision: u64,
}

impl Default for Filter {
    fn default() -> Self {
        Self { input: None, text: String::new(), expanded: false, hover_suppressed: false, from: 0.0, started: std::time::Instant::now(), revision: 0 }
    }
}

fn slide_progress(from: f32, expanded: bool, elapsed: f32) -> f32 {
    let t = elapsed.clamp(0.0, 1.0);
    let eased = t * t * (3.0 - 2.0 * t);
    from + (if expanded { 1.0 } else { 0.0 } - from) * eased
}

impl Filter {
    fn set_expanded(&mut self, expanded: bool) {
        if self.expanded == expanded {
            return;
        }
        self.from = slide_progress(self.from, self.expanded, self.started.elapsed().as_secs_f32() / SLIDE);
        self.expanded = expanded;
        self.started = std::time::Instant::now();
        self.revision = self.revision.wrapping_add(1);
    }

    /// Back to no filter and no field, as a new folder starts.
    pub(super) fn clear(&mut self) {
        *self = Self { hover_suppressed: self.hover_suppressed, ..Self::default() };
    }
}

impl FileChooserWindow {
    /// Slides the field out and gives it the keyboard.
    pub(super) fn open_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(input) = &self.filter.input {
            let focus = input.read(cx).focus_handle.clone();
            self.filter.set_expanded(true);
            window.focus(&focus, cx);
            cx.notify();
            return;
        }
        let this = cx.entity().downgrade();
        let (change, enter, escape) = (this.clone(), this.clone(), this);
        let input = cx.new(|cx| TextInput::new(
            cx,
            cx.i18n().text(Message::NameFilterPlaceholder),
            self.filter.text.clone(),
            move |text, _, cx| { let _ = change.update(cx, |this, cx| this.set_filter(text, cx)); },
            move |window, cx| { let _ = enter.update(cx, |this, cx| this.accept(window, cx)); },
            move |window, cx| {
                let _ = escape.update(cx, |this, cx| {
                    this.clear_filter(cx);
                    window.focus(&this.focus, cx);
                });
            },
            |_, _, _, _, _| {},
            TextInputOptions { appearance: InputAppearance::Bare, ..Default::default() },
            window,
        ));
        let focus = input.read(cx).focus_handle.clone();
        self.filter.input = Some(input);
        self.filter.from = 0.0;
        self.filter.expanded = false;
        self.filter.set_expanded(true);
        window.focus(&focus, cx);
        cx.notify();
    }

    fn collapse_filter(&mut self, cx: &mut Context<Self>) {
        self.filter.set_expanded(false);
        cx.notify();
    }

    /// Escape: the filter and its field go, every name shows again.
    fn clear_filter(&mut self, cx: &mut Context<Self>) {
        let filtered = !self.filter.text.is_empty();
        self.filter.text.clear();
        self.filter.input = None;
        self.filter.set_expanded(false);
        if filtered {
            let keep = self.selected_path();
            self.arrange(keep, cx);
        }
        cx.notify();
    }

    pub(super) fn render_filter_control(&self, theme: OxydeTheme, cx: &mut Context<Self>) -> AnyElement {
        let expanded = self.filter.expanded && self.filter.input.is_some();
        let (from, revision) = (self.filter.from, self.filter.revision);
        div()
            .id("file-chooser-filter-control")
            .flex()
            .flex_none()
            .items_center()
            .child(div().flex_none().w(rems(0.0625)).h(rems(1.0)).mr(rems(0.25)).bg(theme.border))
            .when_some(self.filter.input.clone(), |bar, input| {
                let field = div()
                    .flex_none()
                    .overflow_hidden()
                    .h(rems(1.5))
                    // Its arrows move the list's selection, so Enter chooses what they reach.
                    .on_action(cx.listener(|this, _: &CompletionUp, _, cx| this.move_selection("up", cx)))
                    .on_action(cx.listener(|this, _: &CompletionDown, _, cx| this.move_selection("down", cx)))
                    .child(div().w(rems(10.0)).h_full().pl(rems(0.25)).child(input));
                bar.child(field.with_animation(
                    SharedString::from(format!("file-chooser-filter-slide-{revision}")),
                    Animation::new(std::time::Duration::from_secs_f32(SLIDE)),
                    move |field, delta| {
                        let fraction = slide_progress(from, expanded, delta);
                        field.w(rems(10.0 * fraction)).opacity(fraction)
                    },
                ))
            })
            .child(
                div()
                    .id("file-chooser-filter")
                    .size(rems(1.5))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_sm()
                    .hover(move |style| style.bg(theme.control_hover))
                    // The field's placeholder names the action; a tooltip has no trailing ellipsis.
                    .fixed_tooltip(Tooltip::with_shortcut(cx.i18n().text(Message::NameFilterPlaceholder).trim_end_matches(['…', '.']).to_string(), Some("Ctrl+F".into())))
                    .child(svg().path("icons/search.svg").size(rems(1.0)).text_color(theme.text))
                    .on_hover(cx.listener(|this, hovered: &bool, window, cx| {
                        if !*hovered {
                            this.filter.hover_suppressed = false;
                        } else if !this.filter.expanded && !this.filter.hover_suppressed {
                            this.open_filter(window, cx);
                        }
                    }))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if expanded {
                            this.filter.hover_suppressed = true;
                            this.collapse_filter(cx);
                            window.focus(&this.focus, cx);
                        } else {
                            this.open_filter(window, cx);
                        }
                    })),
            )
            .when(expanded, |control| {
                control.on_mouse_down_out(cx.listener(|this, _, window, cx| {
                    let focused = this.filter.input.as_ref().is_some_and(|input| input.read(cx).focus_handle.is_focused(window));
                    this.collapse_filter(cx);
                    if focused {
                        window.focus(&this.focus, cx);
                    }
                }))
            })
            .into_any_element()
    }
}
