//! One Oxyde per session, as Nautilus is one process: a later launch hands its locations
//! to the running Oxyde over the session bus and ends. While Oxyde is the default for
//! folders, the running one also answers `org.freedesktop.FileManager1`, which browsers
//! call for « Show in folder », and a user D-Bus service file starts Oxyde for it.
use super::{InstanceClaim, InstanceMessage, InstanceRequest};
use futures::channel::mpsc::{unbounded, UnboundedSender};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use zbus::blocking::Connection;
use zbus::fdo::{RequestNameFlags, RequestNameReply};

/// Debug builds answer under their own name: a `cargo run` never hands its locations to
/// an installed Oxyde, nor receives the installed one's.
const NAME: &str = if cfg!(debug_assertions) { "io.github.Carlton2001.Oxyde.Devel" } else { "io.github.Carlton2001.Oxyde" };
const PATH: &str = "/io/github/Carlton2001/Oxyde";
const INTERFACE: &str = "io.github.Carlton2001.Oxyde1";
const FILE_MANAGER: &str = "org.freedesktop.FileManager1";
const FILE_MANAGER_PATH: &str = "/org/freedesktop/FileManager1";
/// Named after Oxyde rather than the interface, so it never replaces another program's.
const SERVICE_FILE: &str = "io.github.Carlton2001.Oxyde.FileManager1.service";

/// The first instance's connection, which holds its names until the process ends.
static CONNECTION: OnceLock<Connection> = OnceLock::new();

fn send(sender: &UnboundedSender<InstanceMessage>, request: InstanceRequest, token: String) {
    let activation_token = Some(token).filter(|token| !token.is_empty());
    if sender.unbounded_send(InstanceMessage { request, activation_token }).is_err() {
        log::warn!("A request for the running Oxyde arrived after its window closed");
    }
}

struct Instance(UnboundedSender<InstanceMessage>);

#[zbus::interface(name = "io.github.Carlton2001.Oxyde1")]
impl Instance {
    /// Locations a later launch received; none brings the window forward.
    fn open(&self, locations: Vec<String>, activation_token: String) {
        send(&self.0, InstanceRequest::Folders(super::linux_startup::folders(&locations)), activation_token);
    }
}

/// The interface Nautilus, Dolphin and Nemo implement, with URIs and a startup token.
struct FileManager(UnboundedSender<InstanceMessage>);

#[zbus::interface(name = "org.freedesktop.FileManager1")]
impl FileManager {
    fn show_folders(&self, uris: Vec<String>, startup_id: String) {
        send(&self.0, InstanceRequest::Folders(super::linux_startup::folders(&uris)), startup_id);
    }

    fn show_items(&self, uris: Vec<String>, startup_id: String) {
        send(&self.0, InstanceRequest::Items(super::linux_startup::items(&uris)), startup_id);
    }

    fn show_item_properties(&self, uris: Vec<String>, startup_id: String) {
        send(&self.0, InstanceRequest::Properties(super::linux_startup::items(&uris)), startup_id);
    }
}

/// The token this launch received, which lets the running window come forward.
fn launch_token() -> String {
    ["XDG_ACTIVATION_TOKEN", "DESKTOP_STARTUP_ID"].into_iter()
        .filter_map(|name| std::env::var(name).ok())
        .find(|token| !token.is_empty())
        .unwrap_or_default()
}

pub(super) fn claim(locations: &[String]) -> InstanceClaim {
    let (sender, receiver) = unbounded();
    let connection = zbus::blocking::connection::Builder::session()
        .and_then(|builder| builder.serve_at(PATH, Instance(sender.clone())))
        .and_then(|builder| builder.serve_at(FILE_MANAGER_PATH, FileManager(sender)))
        // A running Oxyde that does not answer must not hold this launch for long.
        .map(|builder| builder.method_timeout(std::time::Duration::from_secs(5)))
        .and_then(|builder| builder.build());
    let connection = match connection {
        Ok(connection) => connection,
        Err(error) => {
            log::warn!("No session bus, Oxyde runs alone: {error}");
            return InstanceClaim::Alone;
        }
    };
    match connection.request_name_with_flags(NAME, RequestNameFlags::DoNotQueue.into()) {
        Ok(RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner) => {
            let _ = CONNECTION.set(connection);
            std::thread::Builder::new().name("file-manager-service".into())
                .spawn(follow_folder_default)
                .map_err(|error| log::warn!("Cannot check the FileManager1 service: {error}"))
                .ok();
            InstanceClaim::First(receiver)
        }
        // zbus reports DoNotQueue's « exists » answer as NameTaken.
        Ok(RequestNameReply::Exists | RequestNameReply::InQueue) | Err(zbus::Error::NameTaken) => {
            let token = launch_token();
            match connection.call_method(Some(NAME), PATH, Some(INTERFACE), "Open", &(locations, token)) {
                Ok(_) => InstanceClaim::Forwarded,
                Err(error) => {
                    log::warn!("The running Oxyde did not take the locations, this one opens them: {error}");
                    InstanceClaim::Alone
                }
            }
        }
        Err(error) => {
            log::warn!("Cannot claim {NAME}, Oxyde runs alone: {error}");
            InstanceClaim::Alone
        }
    }
}

/// At startup, the service follows the folder default, which another program or
/// install.sh may have changed since.
fn follow_folder_default() {
    if let Some(enabled) = super::linux_gio::applications::opens_folders() {
        serve_file_manager(enabled);
    }
}

fn service_path() -> Option<PathBuf> {
    Some(dirs::data_dir()?.join("dbus-1/services").join(SERVICE_FILE))
}

/// The D-Bus service file that starts `executable` when a program calls FileManager1.
pub(super) fn service_file(executable: &Path) -> String {
    // The bus splits Exec as a shell would: quote the path, escaping what quotes cannot hold.
    let quoted = executable.to_string_lossy().replace('\\', "\\\\").replace('"', "\\\"")
        .replace('$', "\\$").replace('`', "\\`");
    format!("[D-BUS Service]\nName={FILE_MANAGER}\nExec=\"{quoted}\"\n")
}

/// Whether the file changed.
fn write_service(executable: &Path) -> std::io::Result<bool> {
    let path = service_path().ok_or_else(|| std::io::Error::other("No user data folder"))?;
    let contents = service_file(executable);
    if std::fs::read_to_string(&path).is_ok_and(|current| current == contents) {
        return Ok(false);
    }
    let folder = path.parent().ok_or_else(|| std::io::Error::other("No services folder"))?;
    std::fs::create_dir_all(folder)?;
    let mut temporary = tempfile::NamedTempFile::new_in(folder)?;
    std::io::Write::write_all(&mut temporary, contents.as_bytes())?;
    temporary.persist(&path).map_err(|error| error.error)?;
    Ok(true)
}

/// Whether there was a file to remove.
fn remove_service() -> std::io::Result<bool> {
    match service_path().map(std::fs::remove_file) {
        Some(Ok(())) => Ok(true),
        Some(Err(error)) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
        _ => Ok(false),
    }
}

/// Answers FileManager1 and lets the bus start Oxyde for it, or gives both back. A program
/// already holding the name keeps it: Oxyde waits in the bus's queue until it lets go.
pub(super) fn serve_file_manager(enabled: bool) {
    let written = if enabled {
        super::linux_gio::applications::oxyde_executable()
            .ok_or_else(|| std::io::Error::other("Oxyde's desktop entry names no program"))
            .and_then(|executable| write_service(&executable))
    } else {
        remove_service()
    };
    let Some(connection) = CONNECTION.get() else { return };
    match written {
        // dbus-broker reads service files at startup and on reload only.
        Ok(changed) => if changed {
            if let Err(error) = zbus::blocking::fdo::DBusProxy::new(connection).and_then(|bus| Ok(bus.reload_config()?)) {
                log::warn!("The session bus did not reload its services: {error}");
            }
        },
        Err(error) => log::warn!("Cannot update the FileManager1 service file: {error}"),
    }
    let result = if enabled {
        connection.request_name_with_flags(FILE_MANAGER, RequestNameFlags::AllowReplacement.into())
            .map(|reply| log::info!("{FILE_MANAGER}: {reply:?}"))
    } else {
        connection.release_name(FILE_MANAGER).map(drop)
    };
    if let Err(error) = result {
        log::warn!("Cannot change the {FILE_MANAGER} name: {error}");
    }
}

#[cfg(test)]
#[path = "linux_instance/tests/service.rs"]
mod tests;
