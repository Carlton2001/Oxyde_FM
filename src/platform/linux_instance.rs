//! One Oxyde per session, as Nautilus is one process: a later launch hands its locations
//! to the running Oxyde over the session bus and ends. While Oxyde is the default for
//! folders, the running one also answers `org.freedesktop.FileManager1`, which browsers
//! call for « Show in folder », and a user D-Bus service file starts Oxyde for it. The
//! same connection answers for the file chooser portal (`linux_portal`).
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

    /// The bus started a second Oxyde for the file chooser portal: this one answers instead.
    async fn serve_file_chooser(&self, #[zbus(connection)] connection: &zbus::Connection) {
        let flags = RequestNameFlags::AllowReplacement.into();
        match connection.request_name_with_flags(super::linux_portal::NAME, flags).await {
            Ok(reply) => log::info!("{}: {reply:?}", super::linux_portal::NAME),
            Err(error) => log::warn!("Cannot answer for the file chooser portal: {error}"),
        }
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
        .and_then(|builder| builder.serve_at(FILE_MANAGER_PATH, FileManager(sender.clone())))
        .and_then(|builder| builder.serve_at(super::linux_portal::PATH, super::linux_portal::FileChooser(sender)))
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
            // The bus holds the portal's call until this name has an owner.
            if super::linux_portal::launched() {
                super::linux_portal::serve(true);
            }
            std::thread::Builder::new().name("file-manager-service".into())
                .spawn(follow_folder_default)
                .map_err(|error| log::warn!("Cannot check the FileManager1 service: {error}"))
                .ok();
            InstanceClaim::First(receiver)
        }
        // zbus reports DoNotQueue's « exists » answer as NameTaken.
        Ok(RequestNameReply::Exists | RequestNameReply::InQueue) | Err(zbus::Error::NameTaken) => {
            if super::linux_portal::launched() {
                if let Err(error) = connection.call_method(Some(NAME), PATH, Some(INTERFACE), "ServeFileChooser", &()) {
                    log::warn!("The running Oxyde does not answer for the file chooser portal: {error}");
                }
                return InstanceClaim::Forwarded;
            }
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

/// At startup, the services follow the folder default and the portal's configuration,
/// which another program or install.sh may have changed since.
fn follow_folder_default() {
    if let Some(enabled) = super::linux_gio::applications::opens_folders() {
        serve_file_manager(enabled);
    }
    super::linux_portal::follow();
}

/// The session's connection, once this process is the session's Oxyde.
pub(super) fn connection() -> Option<&'static Connection> {
    CONNECTION.get()
}

fn service_path(file_name: &str) -> Option<PathBuf> {
    Some(dirs::data_dir()?.join("dbus-1/services").join(file_name))
}

/// The D-Bus service file that starts `executable`, with `argument`, for the bus name `name`.
pub(super) fn service_file(name: &str, executable: &Path, argument: Option<&str>) -> String {
    // The bus splits Exec as a shell would: quote the path, escaping what quotes cannot hold.
    let quoted = executable.to_string_lossy().replace('\\', "\\\\").replace('"', "\\\"")
        .replace('$', "\\$").replace('`', "\\`");
    let argument = argument.map(|argument| format!(" {argument}")).unwrap_or_default();
    format!("[D-BUS Service]\nName={name}\nExec=\"{quoted}\"{argument}\n")
}

/// Whether the file changed.
pub(super) fn write_service(file_name: &str, contents: &str) -> std::io::Result<bool> {
    let path = service_path(file_name).ok_or_else(|| std::io::Error::other("No user data folder"))?;
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
pub(super) fn remove_service(file_name: &str) -> std::io::Result<bool> {
    match service_path(file_name).map(std::fs::remove_file) {
        Some(Ok(())) => Ok(true),
        Some(Err(error)) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
        _ => Ok(false),
    }
}

/// dbus-broker reads service files at startup and on reload only.
pub(super) fn reload_services() {
    let Some(connection) = CONNECTION.get() else { return };
    if let Err(error) = zbus::blocking::fdo::DBusProxy::new(connection).and_then(|bus| Ok(bus.reload_config()?)) {
        log::warn!("The session bus did not reload its services: {error}");
    }
}

/// Answers for `name` on the session's connection, or lets it go. A program already
/// holding the name keeps it: Oxyde waits in the bus's queue until it lets go.
pub(super) fn own_name(name: &str, enabled: bool) {
    let Some(connection) = CONNECTION.get() else { return };
    let result = if enabled {
        connection.request_name_with_flags(name, RequestNameFlags::AllowReplacement.into())
            .map(|reply| log::info!("{name}: {reply:?}"))
    } else {
        connection.release_name(name).map(drop)
    };
    if let Err(error) = result {
        log::warn!("Cannot change the {name} name: {error}");
    }
}

/// Answers FileManager1 and lets the bus start Oxyde for it, or gives both back.
pub(super) fn serve_file_manager(enabled: bool) {
    let written = if enabled {
        super::linux_gio::applications::oxyde_executable()
            .ok_or_else(|| std::io::Error::other("Oxyde's desktop entry names no program"))
            .and_then(|executable| write_service(SERVICE_FILE, &service_file(FILE_MANAGER, &executable, None)))
    } else {
        remove_service(SERVICE_FILE)
    };
    match written {
        Ok(changed) => if changed { reload_services() },
        Err(error) => log::warn!("Cannot update the FileManager1 service file: {error}"),
    }
    own_name(FILE_MANAGER, enabled);
}

#[cfg(test)]
#[path = "linux_instance/tests/service.rs"]
mod tests;
