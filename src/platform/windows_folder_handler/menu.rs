//! "Open in Oxyde" in the menus of folders, for whoever keeps another file manager as
//! the default: a verb of `Directory`, of its background and of `Drive`, written for
//! the user under `HKCU\Software\Classes` and running Oxyde itself on the folder. Only
//! the copy the Setup installed lists itself, and only while it is not the default,
//! when opening a folder already shows it in Oxyde; its uninstaller removes the verb. A
//! portable copy, deleted or moved, would leave an entry that starts nothing.
use super::{open, remove_empty, text, write, Value};
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, KEY_READ};

/// The verb's key name; its label is the user's language's.
const VERB: &str = "Oxyde";
const CLASSES: [&str; 3] = ["Directory", r"Directory\Background", "Drive"];
/// Where the Setup records the folder it installed Oxyde in.
const UNINSTALL: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Oxyde.GPUI.Windows_is1";

/// The verb's values, running `program` under `label`; none exists when `label` is None.
fn values(program: &str, label: Option<&str>) -> Vec<Value> {
    let value = |key: String, name: &str, data: Option<String>| Value { key, name: name.into(), data, expand: false };
    CLASSES.iter().flat_map(|class| {
        let verb = format!(r"{class}\shell\{VERB}");
        [
            value(verb.clone(), "", label.map(str::to_owned)),
            value(verb.clone(), "Icon", label.map(|_| program.to_owned())),
            value(format!(r"{verb}\command"), "", label.map(|_| format!("\"{program}\" \"%V\""))),
        ]
    }).collect()
}

/// Whether the Setup installed the program in `folder`.
fn installed_in(recorded: Option<&str>, folder: &str) -> bool {
    let plain = |path: &str| path.trim_end_matches('\\').to_lowercase();
    recorded.is_some_and(|recorded| plain(recorded) == plain(folder))
}

/// Whether the entry is listed: by the installed copy, while folders open elsewhere.
fn listed(installed: bool, default: Option<bool>) -> bool {
    installed && default == Some(false)
}

fn program() -> Option<String> {
    let program = std::env::current_exe().ok()?.into_os_string().into_string().ok()?;
    let program = program.strip_prefix(r"\\?\").unwrap_or(&program);
    // A quote cannot be part of a Windows file name; `%` would be read as a parameter.
    (!program.contains(['"', '%'])).then(|| program.to_owned())
}

fn installed(program: &str) -> bool {
    let recorded = open(HKEY_CURRENT_USER, UNINSTALL, KEY_READ).and_then(|key| text(&key, "InstallLocation")).map(|(folder, _)| folder);
    std::path::Path::new(program).parent().and_then(|folder| folder.to_str()).is_some_and(|folder| installed_in(recorded.as_deref(), folder))
}

fn apply(values: &[Value]) {
    for value in values {
        if let Err(error) = write(value) {
            log::warn!("Cannot change the menu of folders at {}: {error}", value.key);
        }
    }
    values.iter().filter(|value| value.data.is_none()).for_each(|value| remove_empty(&value.key));
}

/// Lists Oxyde in the menus of folders under `label`, or removes it, as this copy and
/// the default for folders call for.
pub(in crate::platform) fn refresh(label: &str) {
    let Some(program) = program() else { return };
    let installed = installed(&program);
    // Another copy's entry is not this one's to remove.
    if !installed {
        return;
    }
    apply(&values(&program, listed(installed, super::enabled()).then_some(label)));
}

/// Removes Oxyde from the menus of folders, as the uninstaller asks.
pub(in crate::platform) fn withdraw() {
    apply(&values("", None));
}

#[cfg(test)]
#[path = "tests/menu.rs"]
mod tests;
