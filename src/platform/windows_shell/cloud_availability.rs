//! Resolve availability commands against the Shell menu that will execute them.

use super::{invoke_menu_with, with_menu_snapshot};
use crate::models::{CloudAvailability, CommandError, WinMenuItem};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Shell::IContextMenu;

const ACTIONS: [CloudAvailability; 2] = [CloudAvailability::Offline, CloudAvailability::OnlineOnly];

pub(in crate::platform) fn available(path: &str) -> Result<Vec<CloudAvailability>, CommandError> {
    with_commands(path, |_, commands| {
        Ok(ACTIONS
            .into_iter()
            .zip(commands)
            .filter_map(|(action, command)| command.map(|_| action))
            .collect())
    })
}

pub(in crate::platform) fn execute(
    path: &str,
    availability: CloudAvailability,
) -> Result<(), CommandError> {
    with_commands(path, |menu, commands| {
        let command = ACTIONS
            .into_iter()
            .zip(commands)
            .find_map(|(action, command)| (action == availability).then_some(command).flatten())
            .ok_or_else(|| {
                CommandError::SystemError(
                    "The cloud provider no longer offers this availability command".into(),
                )
            })?;
        invoke_menu_with(command, HWND::default(), |info| unsafe {
            menu.InvokeCommand(info)
        })
    })
}

fn with_commands<T>(
    path: &str,
    read: impl FnOnce(&IContextMenu, [Option<u32>; 2]) -> Result<T, CommandError>,
) -> Result<T, CommandError> {
    let path_on_disk = super::super::windows_shell_selection::shell_path(path)?;
    let google = path_on_disk
        .parent()
        .is_some_and(super::super::windows_cloud_status::is_google_volume);
    with_menu_snapshot(&[path.to_owned()], false, None, true, |menu, items| {
        let find = if google {
            google_command
        } else {
            cloud_files_command
        };
        read(
            menu,
            ACTIONS.map(|action| find(&items, action).map(|item| item.id as u32)),
        )
    })
}

fn cloud_files_command(
    items: &[WinMenuItem],
    availability: CloudAvailability,
) -> Option<&WinMenuItem> {
    let verb = match availability {
        CloudAvailability::Offline => "MakeAvailableOffline",
        CloudAvailability::OnlineOnly => "MakeAvailableOnline",
    };
    // Windows can offer both the modern and legacy entry with the same verb.
    items.iter().find(|item| {
        !item.has_submenu
            && item
                .verb
                .as_deref()
                .is_some_and(|name| name.eq_ignore_ascii_case(verb))
    })
}

fn google_command(items: &[WinMenuItem], availability: CloudAvailability) -> Option<&WinMenuItem> {
    // Drive's two choices publish no canonical verbs. Keep their labels tied to
    // the same language and the dedicated submenu on a Google streaming volume.
    const LABELS: &[(&str, &str, &[&str])] = &[
        ("Offline access", "Available offline", &["Online only"]),
        (
            "Accès hors connexion",
            "Disponible hors connexion",
            &["En ligne uniquement"],
        ),
        (
            "Offlinezugriff",
            "Offline verfügbar",
            &["Nur online", "Nur online verfügbar"],
        ),
        (
            "Acceso sin conexión",
            "Disponible sin conexión",
            &["Solo online", "Solo en línea"],
        ),
        (
            "Accesso offline",
            "Disponibile offline",
            &["Solo online", "Disponibile solo online"],
        ),
    ];
    let mut matches = items
        .iter()
        .filter(|item| item.has_submenu)
        .flat_map(|item| {
            let labels = LABELS
                .iter()
                .find(|(submenu, _, _)| label_is(&item.label, submenu));
            item.children.iter().filter(move |child| {
                !child.has_submenu
                    && labels.is_some_and(|(_, offline, online)| match availability {
                        CloudAvailability::Offline => label_is(&child.label, offline),
                        CloudAvailability::OnlineOnly => {
                            online.iter().any(|label| label_is(&child.label, label))
                        }
                    })
            })
        });
    let command = matches.next()?;
    matches.next().is_none().then_some(command)
}

fn label_is(label: &str, expected: &str) -> bool {
    let label = label.split_whitespace().collect::<Vec<_>>().join(" ");
    label.to_lowercase() == expected.to_lowercase()
}

#[cfg(test)]
#[path = "tests/cloud_availability.rs"]
mod tests;
