//! Oxyde as the file chooser applications reach through xdg-desktop-portal: Firefox,
//! Chrome, Electron and Flatpak applications. Three files make it so, as for any portal
//! backend installed for one user: a `.portal` file naming Oxyde's bus name, a D-Bus
//! service file that starts Oxyde without a main window when nothing serves that name,
//! and the FileChooser line of the user's `portals.conf`, which puts Oxyde before the
//! portals it named. Those stay the fallback, and get the line back when the choice is
//! turned off. The portal reads these files when it starts only: each change restarts it.
mod chooser;
mod conf;

pub(super) use chooser::{answering, wait_for_replies, FileChooser, PATH};
use std::path::{Path, PathBuf};

/// Debug builds are a portal of their own, as they are an instance of their own.
const ID: &str = if cfg!(debug_assertions) { "oxyde-devel" } else { "oxyde" };
pub(super) const NAME: &str = if cfg!(debug_assertions) {
    "org.freedesktop.impl.portal.desktop.oxyde.Devel"
} else {
    "org.freedesktop.impl.portal.desktop.oxyde"
};
const INTERFACE: &str = "org.freedesktop.impl.portal.FileChooser";
/// The argument the service file starts Oxyde with.
pub(super) const LAUNCH: &str = "--portal";
const FOLDER: &str = "xdg-desktop-portal";
const SERVICE: &str = "xdg-desktop-portal.service";

pub(super) fn launched() -> bool {
    std::env::args_os().skip(1).any(|argument| argument == LAUNCH)
}

/// Where xdg-desktop-portal reads its configuration and its portals, for one session.
struct Places {
    config_home: PathBuf,
    config_dirs: Vec<PathBuf>,
    data_home: PathBuf,
    data_dirs: Vec<PathBuf>,
    /// The portal's own `sysconfdir` and `datadir`, which it reads after the others.
    built_in: [PathBuf; 2],
    /// `XDG_CURRENT_DESKTOP`, lowercased as the portal names its files.
    desktops: Vec<String>,
}

fn search_path(variable: &str, default: &str) -> Vec<PathBuf> {
    let value = std::env::var(variable).ok().filter(|value| !value.is_empty());
    value.as_deref().unwrap_or(default).split(':').map(PathBuf::from)
        .filter(|path| path.is_absolute()).collect()
}

impl Places {
    fn session() -> Option<Self> {
        Some(Self {
            config_home: dirs::config_dir()?,
            config_dirs: search_path("XDG_CONFIG_DIRS", "/etc/xdg"),
            data_home: dirs::data_dir()?,
            data_dirs: search_path("XDG_DATA_DIRS", "/usr/local/share:/usr/share"),
            built_in: ["/etc".into(), "/usr/share".into()],
            desktops: std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().split(':')
                .filter(|desktop| !desktop.is_empty()).map(str::to_ascii_lowercase).collect(),
        })
    }

    /// Folders the portal looks for its configuration in, in its order.
    fn config_folders(&self) -> Vec<PathBuf> {
        let mut folders = vec![self.config_home.join(FOLDER)];
        folders.extend(self.config_dirs.iter().map(|folder| folder.join(FOLDER)));
        folders.push(self.built_in[0].join(FOLDER));
        folders.push(self.data_home.join(FOLDER));
        folders.extend(self.data_dirs.iter().map(|folder| folder.join(FOLDER)));
        folders.push(self.built_in[1].join(FOLDER));
        folders
    }

    /// The desktop's own file first, then the generic one.
    fn config_names(&self) -> Vec<String> {
        self.desktops.iter().map(|desktop| format!("{desktop}-portals.conf"))
            .chain(["portals.conf".to_owned()]).collect()
    }

    fn config_in(&self, folder: &Path) -> Option<PathBuf> {
        self.config_names().into_iter().map(|name| folder.join(name)).find(|path| path.is_file())
    }

    /// The configuration the portal reads: the first folder holding one decides.
    fn config(&self) -> Option<PathBuf> {
        self.config_folders().iter().find_map(|folder| self.config_in(folder))
    }

    /// The user's configuration Oxyde edits: the one they have, or a new one for the desktop.
    fn user_config(&self) -> PathBuf {
        let folder = self.config_home.join(FOLDER);
        self.config_in(&folder).unwrap_or_else(|| folder.join(&self.config_names()[0]))
    }

    fn portal_folders(&self) -> Vec<PathBuf> {
        std::iter::once(&self.data_home).chain(&self.data_dirs).chain([&self.built_in[1]])
            .map(|folder| folder.join(FOLDER).join("portals")).collect()
    }

    fn portal_file(&self) -> PathBuf {
        self.data_home.join(FOLDER).join("portals").join(format!("{ID}.portal"))
    }

    /// Whether the portal would ask Oxyde first for a file.
    fn named(&self) -> bool {
        self.config().and_then(|path| std::fs::read_to_string(path).ok())
            .is_some_and(|text| conf::portals(&text).first().map(String::as_str) == Some(ID))
    }
}

/// The program the bus starts: the desktop entry's, which survives updates, or the
/// build itself during development.
fn executable() -> Option<PathBuf> {
    if cfg!(debug_assertions) {
        std::env::current_exe().ok()
    } else {
        super::linux_gio::applications::oxyde_executable()
    }
}

fn portal_file() -> String {
    format!("[portal]\nDBusName={NAME}\nInterfaces={INTERFACE};\n")
}

fn service_name() -> String {
    format!("{NAME}.service")
}

/// Whether Oxyde chooses files; None without a portal, or a program to start for it.
pub(super) fn state() -> Option<bool> {
    let places = Places::session()?;
    if !places.portal_folders().iter().any(|folder| folder.is_dir()) {
        return None;
    }
    executable()?;
    Some(places.named() && places.portal_file().is_file())
}

/// The portal after Oxyde's for FileChooser, by its bus name.
fn fallback() -> Option<String> {
    let places = Places::session()?;
    let text = std::fs::read_to_string(places.config()?).ok()?;
    let folders = places.portal_folders();
    conf::portals(&text).into_iter().filter(|id| id != ID && id != "*" && id != "none").find_map(|id| {
        folders.iter().find_map(|folder| {
            let text = std::fs::read_to_string(folder.join(format!("{id}.portal"))).ok()?;
            backend_name(&text)
        })
    })
}

/// The bus name a `.portal` file gives, when it implements FileChooser.
fn backend_name(text: &str) -> Option<String> {
    let entry = |key: &str| text.lines().find_map(|line| {
        let (name, value) = line.split_once('=')?;
        (name.trim() == key).then(|| value.trim())
    });
    entry("Interfaces")?.split(';').any(|interface| interface.trim() == INTERFACE)
        .then(|| entry("DBusName").map(str::to_owned)).flatten()
}

/// Writes `contents` atomically in place of the file `path` leads to, links kept.
fn replace(path: &Path, contents: &str) -> std::io::Result<()> {
    let target = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_owned());
    let folder = target.parent().ok_or_else(|| std::io::Error::other("No parent folder"))?;
    std::fs::create_dir_all(folder)?;
    let mut temporary = tempfile::NamedTempFile::new_in(folder)?;
    std::io::Write::write_all(&mut temporary, contents.as_bytes())?;
    temporary.persist(&target).map_err(|error| error.error)?;
    Ok(())
}

/// Whether the file changed.
fn write(path: &Path, contents: &str) -> std::io::Result<bool> {
    if std::fs::read_to_string(path).is_ok_and(|current| current == contents) {
        return Ok(false);
    }
    replace(path, contents)?;
    Ok(true)
}

fn remove(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

/// Puts Oxyde first in the user's configuration, or takes it out. Without a file of
/// their own, the user gets a copy of the one the portal read, which goes again once
/// Oxyde is out of it and nothing else changed.
fn edit_config(places: &Places, enabled: bool) -> std::io::Result<()> {
    let path = places.user_config();
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error),
        Err(_) if !enabled => return Ok(()),
        Err(_) => match places.config() {
            Some(source) => conf::created(&source.to_string_lossy(), &std::fs::read_to_string(&source)?),
            None => String::new(),
        },
    };
    let edited = if enabled { conf::with_portal(&text, ID) } else { conf::without_portal(&text, ID) };
    if !enabled {
        if let Some((source, rest)) = conf::creation(&edited) {
            if std::fs::read_to_string(source).is_ok_and(|original| original.trim_end() == rest.trim_end()) {
                return remove(&path);
            }
        }
    }
    if edited != text {
        replace(&path, &edited)?;
    }
    Ok(())
}

/// Answers for the portal on the session's connection, or lets the name go.
pub(super) fn serve(enabled: bool) {
    super::linux_instance::own_name(NAME, enabled);
}

/// The portal reads its configuration and portals only when it starts.
fn restart_portal() {
    let Some(connection) = super::linux_instance::connection() else { return };
    let restarted = connection.call_method(Some("org.freedesktop.systemd1"), "/org/freedesktop/systemd1",
        Some("org.freedesktop.systemd1.Manager"), "RestartUnit", &(SERVICE, "replace"));
    match restarted {
        Ok(_) => log::info!("{SERVICE} restarted for the file chooser"),
        Err(error) => log::warn!("Cannot restart {SERVICE}; the file chooser changes at the next session: {error}"),
    }
}

pub(super) fn set(enabled: bool) -> Result<(), String> {
    let places = Places::session().ok_or("No user folders")?;
    let failed = |error: std::io::Error| error.to_string();
    if enabled {
        let executable = executable().ok_or("No Oxyde program to start for the portal")?;
        write(&places.portal_file(), &portal_file()).map_err(failed)?;
        let service = super::linux_instance::service_file(NAME, &executable, Some(LAUNCH));
        super::linux_instance::write_service(&service_name(), &service).map_err(failed)?;
        edit_config(&places, true).map_err(failed)?;
    } else {
        edit_config(&places, false).map_err(failed)?;
        remove(&places.portal_file()).map_err(failed)?;
        super::linux_instance::remove_service(&service_name()).map_err(failed)?;
    }
    super::linux_instance::reload_services();
    serve(enabled);
    restart_portal();
    Ok(())
}

/// At startup, while the portal names Oxyde: files an update or a cleanup changed are
/// written again, and this Oxyde answers for the portal.
pub(super) fn follow() {
    let Some(places) = Places::session() else { return };
    if !places.named() {
        return;
    }
    let Some(executable) = executable() else { return };
    let portal_changed = write(&places.portal_file(), &portal_file())
        .unwrap_or_else(|error| { log::warn!("Cannot write the file chooser portal: {error}"); false });
    let service = super::linux_instance::service_file(NAME, &executable, Some(LAUNCH));
    match super::linux_instance::write_service(&service_name(), &service) {
        Ok(true) => super::linux_instance::reload_services(),
        Ok(false) => {}
        Err(error) => log::warn!("Cannot write the file chooser service: {error}"),
    }
    serve(true);
    // Started for a call, which a restart would take from its application.
    if portal_changed && !launched() {
        restart_portal();
    }
}

#[cfg(test)]
#[path = "linux_portal/tests/places.rs"]
mod tests;
