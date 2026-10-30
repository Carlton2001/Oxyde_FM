//! Desktop settings pages, which each Linux desktop opens its own way.
use std::process::{Command, Stdio};

type Program = (&'static str, &'static [&'static str]);

/// NetworkManager's own editor, for desktops without a settings page of their own.
const CONNECTION_EDITOR: Program = ("nm-connection-editor", &[]);

/// What shows the network settings under `desktop` (`XDG_CURRENT_DESKTOP`), best first.
fn network_settings(desktop: &str) -> Vec<Program> {
    let on = |name: &str| desktop.split(':').any(|part| part.eq_ignore_ascii_case(name));
    let mut programs = Vec::new();
    if on("GNOME") {
        programs.push(("gnome-control-center", &["network"][..]));
    }
    if on("KDE") {
        // Plasma 5 still names its settings differently.
        programs.push(("systemsettings", &["kcm_networkmanagement"][..]));
        programs.push(("systemsettings5", &["kcm_networkmanagement"][..]));
    }
    if on("X-Cinnamon") {
        programs.push(("cinnamon-settings", &["network"][..]));
    }
    programs.push(CONNECTION_EDITOR);
    programs
}

/// Opens the network settings and waits for them in the background, so that they do not
/// stay a zombie. The menu entry has no result to show: a failure is only logged.
pub(super) fn open_network_settings() {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    std::thread::spawn(move || {
        for (program, arguments) in network_settings(&desktop) {
            let mut command = Command::new(program);
            super::linux_gpu::leave_out_of(&mut command);
            command.args(arguments).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
            match command.spawn() {
                Ok(mut child) => {
                    let _ = child.wait();
                    return;
                }
                Err(error) => log::debug!("{program}: {error}"),
            }
        }
        log::warn!("No network settings found for desktop {desktop:?}");
    });
}

#[cfg(test)]
mod tests;
