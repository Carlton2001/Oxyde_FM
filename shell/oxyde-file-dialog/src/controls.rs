//! The controls an application adds to its dialog, as far as Oxyde shows them: a
//! combo box or a list of radio buttons becomes a menu of answers, a check button a
//! checkbox. Each one is noted on its way to the system's dialog, which keeps their
//! state: Oxyde is asked with what it holds when shown, and its answers are set back
//! on it, as the user would have set them, before the application reads them.
use crate::request::AskedChoice;
use windows::Win32::UI::Shell::{IFileDialogCustomize, CDCS_ENABLED, CDCS_VISIBLE};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// A combo box or a list of radio buttons.
    Choice,
    Check,
    Text,
    Group,
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
    /// in its group.
    fn label_of(&self, position: usize) -> String {
        let control = &self.list[position];
        if !control.label.is_empty() {
            return control.label.clone();
        }
        let group = control.group.and_then(|group| self.list.iter().find(|known| known.id == group && known.kind == Kind::Group));
        if let Some(group) = group.filter(|group| !group.label.is_empty()) {
            return group.label.clone();
        }
        self.list[..position].iter().rev()
            .take_while(|known| known.group == control.group)
            .find(|known| known.kind == Kind::Text)
            .map(|text| text.label.clone())
            .unwrap_or_default()
    }

    /// The questions Oxyde asks, from the controls the user could change in the
    /// system's dialog, as `inner` holds them.
    pub(crate) unsafe fn asked(&self, inner: &IFileDialogCustomize) -> Vec<AskedChoice> {
        let usable = |state: windows::core::Result<windows::Win32::UI::Shell::CDCONTROLSTATEF>| {
            state.is_ok_and(|state| state.0 & CDCS_VISIBLE.0 != 0 && state.0 & CDCS_ENABLED.0 != 0)
        };
        self.list.iter().enumerate().filter_map(|(position, control)| {
            if !usable(inner.GetControlState(control.id)) {
                return None;
            }
            let group_shown = control.group.is_none_or(|group| usable(inner.GetControlState(group)));
            if !group_shown {
                return None;
            }
            let label = self.label_of(position);
            match control.kind {
                Kind::Check => {
                    let checked = inner.GetCheckButtonState(control.id).is_ok_and(|checked| checked.as_bool());
                    Some(AskedChoice { id: control.id, label, options: Vec::new(), initial: checked.to_string() })
                }
                Kind::Choice => {
                    let options: Vec<(u32, String)> = control.items.iter()
                        .filter(|(item, _)| usable(inner.GetControlItemState(control.id, *item)))
                        .cloned()
                        .collect();
                    let selected = inner.GetSelectedControlItem(control.id).ok().filter(|selected| options.iter().any(|(item, _)| item == selected));
                    let initial = selected.or_else(|| options.first().map(|(item, _)| *item))?;
                    Some(AskedChoice { id: control.id, label, options, initial: initial.to_string() })
                }
                Kind::Text | Kind::Group => None,
            }
        }).collect()
    }

    /// Whether `id` is a check button.
    pub(crate) fn is_check(&self, id: u32) -> bool {
        self.list.iter().any(|control| control.id == id && control.kind == Kind::Check)
    }
}
