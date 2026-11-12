//! The name filter at the end of the navigation bar, a copy of the main window's quick
//! search control (`render_quick_search_control`): a separator, a field sliding out of
//! the magnifier on hover, click or Ctrl+F, collapsing on a click elsewhere. Here it
//! filters the folder's names as typed, and Enter opens or chooses the selection.
use super::FileChooserWindow;
use crate::ui::app::search::{slide_progress, SearchInputSlide};
use crate::ui::components::input::{CompletionDown, CompletionUp, InputAppearance, TextInput, TextInputOptions};
use crate::ui::components::tooltip::{NativeTooltipExt, Tooltip};
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::OxydeTheme;
use gpui::prelude::*;
use gpui::*;

#[derive(Default)]
pub(super) struct Filter {
    pub(super) input: Option<Entity<TextInput>>,
    pub(super) text: String,
    slide: SearchInputSlide,
}

impl Filter {
    /// Back to no filter and no field, as a new folder starts.
    pub(super) fn clear(&mut self) {
        *self = Self { slide: SearchInputSlide { hover_suppressed: self.slide.hover_suppressed, ..Default::default() }, ..Self::default() };
    }
}

impl FileChooserWindow {
    /// Slides the field out and gives it the keyboard.
    pub(super) fn open_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(input) = &self.filter.input {
            let focus = input.read(cx).focus_handle.clone();
            self.filter.slide.set_expanded(true);
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
            move |window, cx| { let _ = enter.update(cx, |this, cx| this.activate(window, cx)); },
            move |window, cx| {
                let _ = escape.update(cx, |this, cx| {
                    this.clear_filter(cx);
                    window.focus(&this.focus, cx);
                });
            },
            |_, _, _, _, _| {},
            TextInputOptions { appearance: InputAppearance::Embedded, ..Default::default() },
            window,
        ));
        let focus = input.read(cx).focus_handle.clone();
        self.filter.input = Some(input);
        self.filter.slide.from = 0.0;
        self.filter.slide.expanded = false;
        self.filter.slide.set_expanded(true);
        window.focus(&focus, cx);
        cx.notify();
    }

    fn collapse_filter(&mut self, cx: &mut Context<Self>) {
        self.filter.slide.set_expanded(false);
        cx.notify();
    }

    /// Escape: the filter and its field go, every name shows again.
    fn clear_filter(&mut self, cx: &mut Context<Self>) {
        let filtered = !self.filter.text.is_empty();
        self.filter.text.clear();
        self.filter.input = None;
        self.filter.slide.set_expanded(false);
        if filtered {
            let keep = self.kept();
            self.arrange(keep, cx);
        }
        cx.notify();
    }

    pub(super) fn render_filter_control(&self, theme: OxydeTheme, cx: &mut Context<Self>) -> AnyElement {
        let expanded = self.filter.slide.expanded && self.filter.input.is_some();
        let (from, revision) = (self.filter.slide.from, self.filter.slide.revision);
        div()
            .id("file-chooser-filter-control")
            .h_full()
            .flex()
            .flex_none()
            .items_center()
            .child(crate::ui::elements::separator::vertical(theme.border))
            .when_some(self.filter.input.clone(), |bar, input| {
                let field = div()
                    .flex_none()
                    .overflow_hidden()
                    .h(crate::ui::components::input::BAR_FIELD_HEIGHT)
                    // Its arrows move the list's selection, so Enter chooses what they reach.
                    .on_action(cx.listener(|this, _: &CompletionUp, _, cx| this.move_selection("up", false, cx)))
                    .on_action(cx.listener(|this, _: &CompletionDown, _, cx| this.move_selection("down", false, cx)))
                    // The one borderless field: the surface alone sets it off in the bar.
                    .child(div().w(rems(10.0)).h_full().rounded(crate::ui::components::input::FIELD_RADIUS).bg(theme.surface).child(input));
                bar.child(field.with_animation(
                    SharedString::from(format!("file-chooser-filter-slide-{revision}")),
                    Animation::new(std::time::Duration::from_millis(250)),
                    move |field, delta| {
                        let fraction = slide_progress(from, expanded, delta);
                        field.w(rems(10.0 * fraction)).opacity(fraction)
                    },
                ))
            })
            .child(
                crate::ui::elements::bar_button::fill(
                    crate::ui::elements::bar_button::cell("file-chooser-filter", crate::ui::style::NAV_BAR_HEIGHT),
                    true, false, theme,
                )
                    // The field's placeholder names the action; a tooltip has no trailing ellipsis.
                    .fixed_tooltip(Tooltip::with_shortcut(cx.i18n().text(Message::NameFilterPlaceholder).trim_end_matches(['…', '.']).to_string(), Some("Ctrl+F".into())))
                    .child(crate::ui::elements::bar_button::icon("icons/search.svg", theme.text))
                    .on_hover(cx.listener(|this, hovered: &bool, window, cx| {
                        if !*hovered {
                            this.filter.slide.hover_suppressed = false;
                        } else if !this.filter.slide.expanded && !this.filter.slide.hover_suppressed {
                            this.open_filter(window, cx);
                        }
                    }))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if expanded {
                            this.filter.slide.hover_suppressed = true;
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
