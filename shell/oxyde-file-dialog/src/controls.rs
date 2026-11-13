//! The controls an application adds to its dialog, as far as Oxyde shows them: a
//! combo box or a list of radio buttons becomes a menu of answers, a check button a
//! checkbox, an edit box a text field, a text naming nothing a note, and the open
//! button's drop-down a menu beside Oxyde's. A control shown but disabled is shown so.
//! Each one is noted on its way to the system's dialog, which keeps their
//! state: Oxyde is asked with what it holds when shown, and its answers are set back
//! on it, as the user would have set them, before the application reads them.
use crate::request::{AskedChoice, AskedField};
use windows::Win32::UI::Shell::{IFileDialogCustomize, CDCS_ENABLED, CDCS_VISIBLE};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// A combo box or a list of radio buttons.
    Choice,
    Check,
    Edit,
    /// The open button's drop-down.
    OpenDropDown,
    Text,
    Group,
}

/// What the controls ask of Oxyde.
#[derive(Default)]
pub(crate) struct AskedControls {
    pub(crate) choices: Vec<AskedChoice>,
    pub(crate) fields: Vec<AskedField>,
    pub(crate) notes: Vec<String>,
    pub(crate) accept_menu: Option<AskedChoice>,
    pub(crate) disabled: Vec<u32>,
}

struct Control {
    id: u32,
    kind: Kind,
    label: String,
    /// The visual group the control was added in.
    group: Option<u32>,
    items: Vec<(u32, String)>,
}

#[derive(Default)]
pub(crate) struct Controls {
    list: Vec<Control>,
    /// The visual group being filled.
    open: Option<u32>,
}

/// A label without its mnemonic: `&File` reads `File`, `&&` an ampersand.
fn plain_label(label: &str) -> String {
    label.replace("&&", "\u{0}").replace('&', "").replace('\u{0}', "&").trim().trim_end_matches(':').trim().to_owned()
}

impl Controls {
    fn add(&mut self, id: u32, kind: Kind, label: &str) {
        self.list.retain(|control| control.id != id);
        self.list.push(Control { id, kind, label: plain_label(label), group: self.open, items: Vec::new() });
    }

    pub(crate) fn add_choice(&mut self, id: u32) {
        self.add(id, Kind::Choice, "");
    }

    pub(crate) fn add_check(&mut self, id: u32, label: &str) {
        self.add(id, Kind::Check, label);
    }

    pub(crate) fn add_edit(&mut self, id: u32) {
        self.add(id, Kind::Edit, "");
    }

    pub(crate) fn add_open_drop_down(&mut self, id: u32) {
        self.add(id, Kind::OpenDropDown, "");
    }

    pub(crate) fn add_text(&mut self, id: u32, text: &str) {
        self.add(id, Kind::Text, text);
    }

    pub(crate) fn start_group(&mut self, id: u32, label: &str) {
        self.open = None;
        self.add(id, Kind::Group, label);
        self.open = Some(id);
    }

    pub(crate) fn end_group(&mut self) {
        self.open = None;
    }

    fn control(&mut self, id: u32) -> Option<&mut Control> {
        self.list.iter_mut().find(|control| control.id == id)
    }

    pub(crate) fn set_label(&mut self, id: u32, label: &str) {
        if let Some(control) = self.control(id) {
            control.label = plain_label(label);
        }
    }

    pub(crate) fn add_item(&mut self, id: u32, item: u32, label: &str) {
        if let Some(control) = self.control(id) {
            control.items.retain(|(known, _)| *known != item);
            control.items.push((item, plain_label(label)));
        }
    }

    pub(crate) fn set_item_text(&mut self, id: u32, item: u32, label: &str) {
        if let Some((_, known)) = self.control(id).and_then(|control| control.items.iter_mut().find(|(known, _)| *known == item)) {
            *known = plain_label(label);
        }
    }

    pub(crate) fn remove_item(&mut self, id: u32, item: u32) {
        if let Some(control) = self.control(id) {
            control.items.retain(|(known, _)| *known != item);
        }
    }

    pub(crate) fn remove_items(&mut self, id: u32) {
        if let Some(control) = self.control(id) {
            control.items.clear();
        }
    }

    /// What names a control: its own label, else its group's, else the text before it
    /// in its group, with that text's position.
    fn label_of(&self, position: usize) -> (String, Option<usize>) {
        let control = &self.list[position];
        if !control.label.is_empty() {
            return (control.label.clone(), None);
        }
        let group = control.group.and_then(|group| self.list.iter().find(|known| known.id == group && known.kind == Kind::Group));
        if let Some(group) = group.filter(|group| !group.label.is_empty()) {
            return (group.label.clone(), None);
        }
        self.list[..position].iter().enumerate().rev()
            .take_while(|(_, known)| known.group == control.group)
            .find(|(_, known)| known.kind == Kind::Text)
            .map(|(text, known)| (known.label.clone(), Some(text)))
            .unwrap_or_default()
    }

    /// The questions Oxyde asks, from the controls shown in the system's dialog, as
    /// `inner` holds them.
    pub(crate) unsafe fn asked(&self, inner: &IFileDialogCustomize) -> AskedControls {
        let state = |state: windows::core::Result<windows::Win32::UI::Shell::CDCONTROLSTATEF>| {
            state.map_or((false, false), |state| (state.0 & CDCS_VISIBLE.0 != 0, state.0 & CDCS_ENABLED.0 != 0))
        };
        let mut asked = AskedControls::default();
        // The texts that name a control rather than speak for themselves.
        let mut naming = Vec::new();
        let mut texts = Vec::new();
        for (position, control) in self.list.iter().enumerate() {
            let (visible, enabled) = state(inner.GetControlState(control.id));
            let group_shown = control.group.is_none_or(|group| state(inner.GetControlState(group)) == (true, true));
            if !visible || !group_shown {
                continue;
            }
            let (label, text) = match control.kind {
                // The drop-down's items speak for themselves, beside the button.
                Kind::Text | Kind::Group | Kind::OpenDropDown => (String::new(), None),
                _ => self.label_of(position),
            };
            let asked_before = asked.choices.len() + asked.fields.len() + usize::from(asked.accept_menu.is_some());
            match control.kind {
                Kind::Check => {
                    let checked = inner.GetCheckButtonState(control.id).is_ok_and(|checked| checked.as_bool());
                    asked.choices.push(AskedChoice { id: control.id, label, options: Vec::new(), initial: checked.to_string() });
                }
                Kind::Choice | Kind::OpenDropDown => {
                    let options: Vec<(u32, String)> = control.items.iter()
                        .filter(|(item, _)| state(inner.GetControlItemState(control.id, *item)) == (true, true))
                        .cloned()
                        .collect();
                    let selected = inner.GetSelectedControlItem(control.id).ok().filter(|selected| options.iter().any(|(item, _)| item == selected));
                    let Some(initial) = selected.or_else(|| options.first().map(|(item, _)| *item)) else { continue };
                    let choice = AskedChoice { id: control.id, label, options, initial: initial.to_string() };
                    if control.kind == Kind::Choice {
                        asked.choices.push(choice);
                    } else {
                        asked.accept_menu = Some(choice);
                    }
                }
                Kind::Edit => {
                    let text = inner.GetEditBoxText(control.id).ok().filter(|text| !text.is_null()).map(|text| {
                        let read = windows::core::PCWSTR(text).to_string().unwrap_or_default();
                        windows::Win32::System::Com::CoTaskMemFree(Some(text.cast()));
                        read
                    }).unwrap_or_default();
                    asked.fields.push(AskedField { id: control.id, label, text });
                }
                Kind::Text => texts.push(position),
                Kind::Group => {}
            }
            let shown = asked.choices.len() + asked.fields.len() + usize::from(asked.accept_menu.is_some()) > asked_before;
            if shown {
                naming.extend(text);
                if !enabled {
                    asked.disabled.push(control.id);
                }
            }
        }
        asked.notes = texts.into_iter().filter(|text| !naming.contains(text))
            .map(|text| self.list[text].label.clone()).filter(|note| !note.is_empty()).collect();
        asked
    }

    /// Whether `id` is a check button.
    pub(crate) fn is_check(&self, id: u32) -> bool {
        self.is(id, Kind::Check)
    }

    /// Whether `id` is an edit box.
    pub(crate) fn is_edit(&self, id: u32) -> bool {
        self.is(id, Kind::Edit)
    }

    fn is(&self, id: u32, kind: Kind) -> bool {
        self.list.iter().any(|control| control.id == id && control.kind == kind)
    }
}
