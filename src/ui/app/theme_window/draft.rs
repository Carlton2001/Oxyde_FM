//! The user theme being edited, previewed live in every window until it is saved or dropped.

use super::{PickerPart, ThemeWindow};
use crate::ui::components::input::{TextInput, TextInputOptions};
use crate::ui::elements::color_picker::{self, Hsv};
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::theme::{CustomTheme, OxydeTheme, ThemeChoice, ThemeId, ThemeSeed};
use gpui::*;
use oxyde_lib::models::custom_theme::{format_hex_color, parse_hex_color};

pub(super) struct ThemeDraft {
    /// The saved theme being changed; `None` for a new one.
    pub(super) id: Option<String>,
    pub(super) base: ThemeId,
    pub(super) name: String,
    pub(super) name_input: Entity<TextInput>,
    /// Same order as `ThemeSeed::colors`.
    pub(super) colors: [u32; 6],
    pub(super) hex_inputs: Vec<Entity<TextInput>>,
    /// Seed edited by the picker.
    pub(super) active: usize,
    /// Kept apart from `colors` so the hue survives grays and black.
    pub(super) hsv: Hsv,
    /// Name and colors as last saved; `None` for a new theme.
    saved: Option<(String, [u32; 6])>,
}

impl ThemeDraft {
    /// A new theme always counts as modified: Cancel abandons it, Save creates it.
    pub(super) fn is_modified(&self) -> bool {
        self.saved.as_ref().is_none_or(|(name, colors)| *name != self.name || *colors != self.colors)
    }
}

/// Label of a built-in theme, as the list shows it.
pub(super) fn builtin_label(id: ThemeId, cx: &App) -> SharedString {
    if id == ThemeId::Monokai {
        return id.name().into();
    }
    let mut args = FluentArgs::new();
    args.set("name", id.name());
    cx.i18n().text_with_args(if id.is_light() { Message::ThemeLight } else { Message::ThemeDark }, &args)
}

impl ThemeWindow {
    /// Edit the selected user theme, or start a new one from the selected theme.
    pub(super) fn start_draft(&mut self, edit: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(owner) = self.owner.upgrade() else {
            return;
        };
        let app = owner.read(cx);
        let (id, base, name, colors) = match &app.theme_choice {
            ThemeChoice::Custom(id) => {
                let Some(theme) = app.custom_themes().iter().find(|theme| &theme.id == id) else {
                    return;
                };
                let colors = theme.colors(ThemeSeed::for_id(theme.base()).colors());
                let name = if edit { theme.name.clone() } else { new_name(theme.name.clone().into(), cx) };
                (edit.then(|| id.clone()), theme.base(), name, colors)
            }
            ThemeChoice::Builtin(id) => (None, *id, new_name(builtin_label(*id, cx), cx), ThemeSeed::for_id(*id).colors()),
        };
        let handle = cx.entity().downgrade();
        let name_input = cx.new(|cx| {
            let change = handle.clone();
            let enter = handle.clone();
            let escape = handle.clone();
            TextInput::new(cx, "", name.clone(),
                move |value, _, cx| {
                    let _ = change.update(cx, |this, cx| {
                        if let Some(draft) = &mut this.draft {
                            draft.name = value.into();
                        }
                        cx.notify();
                    });
                },
                move |_, cx| { let _ = enter.update(cx, |this, cx| this.save_draft(cx)); },
                move |_, cx| { let _ = escape.update(cx, |this, cx| this.discard_draft(cx)); },
                |_, _, _, _, _| {},
                TextInputOptions::default(), window)
        });
        let hex_inputs = (0..6).map(|index| {
            let change = handle.clone();
            let enter = handle.clone();
            let escape = handle.clone();
            cx.new(|cx| TextInput::new(cx, "#rrggbb", format_hex_color(colors[index]),
                move |value, _, cx| {
                    let _ = change.update(cx, |this, cx| {
                        if let Some(color) = parse_hex_color(value) {
                            this.set_color(index, color, false, cx);
                        }
                    });
                },
                move |_, cx| { let _ = enter.update(cx, |this, cx| this.save_draft(cx)); },
                move |_, cx| { let _ = escape.update(cx, |this, cx| this.discard_draft(cx)); },
                |_, _, _, _, _| {},
                TextInputOptions::default(), window))
        }).collect();
        let focus = name_input.read(cx).focus_handle.clone();
        let saved = edit.then(|| (name.clone(), colors));
        self.draft = Some(ThemeDraft {
            saved,
            id, base, name, name_input, colors, hex_inputs,
            active: 5,
            hsv: Hsv::from_rgb(colors[5]),
        });
        window.focus(&focus, cx);
        self.preview(cx);
        cx.notify();
    }

    pub(super) fn select_color(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(draft) = self.draft.as_mut().filter(|draft| draft.active != index) {
            draft.active = index;
            draft.hsv = Hsv::from_rgb(draft.colors[index]);
            cx.notify();
        }
    }

    /// `rewrite` puts the new value in the field, for changes made outside it.
    fn set_color(&mut self, index: usize, color: u32, rewrite: bool, cx: &mut Context<Self>) {
        let Some(draft) = &mut self.draft else {
            return;
        };
        if rewrite {
            draft.hex_inputs[index].update(cx, |input, cx| input.set_content(format_hex_color(color), cx));
        } else if draft.active == index {
            draft.hsv = Hsv::from_rgb(color);
        }
        if draft.colors[index] != color {
            draft.colors[index] = color;
            self.preview(cx);
        }
        cx.notify();
    }

    fn set_hsv(&mut self, hsv: Hsv, cx: &mut Context<Self>) {
        let Some(draft) = &mut self.draft else {
            return;
        };
        draft.hsv = hsv;
        let index = draft.active;
        self.set_color(index, hsv.to_rgb(), true, cx);
    }

    pub(super) fn press_picker(&mut self, part: PickerPart, position: Point<Pixels>, cx: &mut Context<Self>) {
        self.drag = Some(part);
        self.drag_picker(position, cx);
    }

    pub(super) fn drag_picker(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let (Some(part), Some(draft)) = (self.drag, &self.draft) else {
            return;
        };
        let hsv = draft.hsv;
        match part {
            PickerPart::Field => if let Some(bounds) = self.field_bounds.get() {
                self.set_hsv(color_picker::field_value(bounds, position, hsv.h), cx);
            },
            PickerPart::Hue => if let Some(bounds) = self.hue_bounds.get() {
                self.set_hsv(Hsv { h: color_picker::hue_value(bounds, position), ..hsv }, cx);
            },
        }
    }

    pub(super) fn preview(&mut self, cx: &mut Context<Self>) {
        if let Some(draft) = &self.draft {
            let colors = draft.colors;
            let _ = self.owner.update(cx, |app, cx| {
                let theme = OxydeTheme::from_colors(colors, app.color_vision());
                app.preview_theme(theme, cx)
            });
        }
    }

    /// The saved theme becomes the selection and stays open in the editor.
    pub(super) fn save_draft(&mut self, cx: &mut Context<Self>) {
        let Some(draft) = self.draft.as_ref().filter(|draft| draft.is_modified()) else {
            return;
        };
        let (id, base, colors) = (draft.id.clone(), draft.base, draft.colors);
        let name = draft.name.trim().to_string();
        let saved = self.owner.update(cx, |app, cx| {
            let id = id.unwrap_or_else(|| CustomTheme::next_id(app.custom_themes()));
            let name = if name.is_empty() { builtin_label(base, cx).to_string() } else { name };
            app.save_custom_theme(CustomTheme::new(id.clone(), name, base, colors), cx);
            id
        });
        if let (Ok(id), Some(draft)) = (saved, &mut self.draft) {
            draft.id = Some(id);
            draft.saved = Some((draft.name.clone(), draft.colors));
        }
        self.drag = None;
        cx.notify();
    }

    /// Selecting a user theme opens its colors; any other selection closes the editor.
    pub(super) fn choose(&mut self, choice: ThemeChoice, window: &mut Window, cx: &mut Context<Self>) {
        self.discard_draft(cx);
        let custom = matches!(choice, ThemeChoice::Custom(_));
        let _ = self.owner.update(cx, |app, cx| app.set_theme(choice, cx));
        if custom {
            self.start_draft(true, window, cx);
        }
    }

    /// Drop unsaved changes; a saved theme reopens as saved.
    pub(super) fn revert_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let saved = self.draft.as_ref().is_some_and(|draft| draft.id.is_some());
        self.discard_draft(cx);
        if saved {
            self.start_draft(true, window, cx);
        }
    }

    pub(super) fn delete_draft(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.draft.as_ref().and_then(|draft| draft.id.clone()) else {
            return;
        };
        self.draft = None;
        self.drag = None;
        let _ = self.owner.update(cx, |app, cx| app.delete_custom_theme(&id, cx));
        cx.notify();
    }

    pub(super) fn discard_draft(&mut self, cx: &mut Context<Self>) {
        if self.draft.take().is_some() {
            let _ = self.owner.update(cx, |app, cx| app.apply_theme(cx));
        }
        self.drag = None;
        cx.notify();
    }
}

fn new_name(source: SharedString, cx: &App) -> String {
    let mut args = FluentArgs::new();
    args.set("name", source.to_string());
    cx.i18n().text_with_args(Message::ThemeNewName, &args).to_string()
}
