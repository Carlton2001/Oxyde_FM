//! Confines the self-workers as GNOME confines its thumbnailers and glycin its image
//! loaders: bubblewrap gives each an empty root with the system read-only, no network,
//! no other process, and a seccomp filter. The document arrives as an open descriptor
//! mounted alone. Without a usable bubblewrap, Landlock and the same filter confine them
//! as far as they can, and the log says so.
use std::ffi::OsString;
use std::fs::File;
use std::io;
use std::os::fd::{AsRawFd, BorrowedFd, OwnedFd, RawFd};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

mod landlock;
mod seccomp;

const BWRAP: &str = "bwrap";
/// Where a confined worker finds its document.
const SOURCE: &str = "/oxyde/source";

/// What a worker may reach beyond the system's read-only files.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Profile {
    /// Pure codecs: archives, Office, audio tags, raster thumbnails.
    Codec,
    /// PDF and images, whose SVGs need them: the user's fonts and their caches.
    Document,
    /// Hover videos: Oxyde's GStreamer registry, read-only.
    Video,
    /// The Viewer's player: the registry and the desktop's sound server.
    Player,
    /// Writes Oxyde's GStreamer registry; it opens no document.
    Registry,
}

impl Profile {
    pub(super) fn of(mode: &str) -> Self {
        match mode {
            "video" => Self::Video,
            "viewer-media" => Self::Player,
            "media-registry" => Self::Registry,
            mode if mode.starts_with("pdf") || mode.starts_with("image") => Self::Document,
            _ => Self::Codec,
        }
    }

    pub(super) fn reads_media(self) -> bool {
        matches!(self, Self::Video | Self::Player)
    }
}

/// A worker command ready for its own arguments. The descriptors it passes stay open
/// until it has started.
pub(super) struct Launch {
    pub(super) command: Command,
    /// The document's path as the worker sees it.
    pub(super) source: Option<OsString>,
    _descriptors: Vec<OwnedFd>,
}

pub(super) fn command(executable: &Path, profile: Profile, source: Option<(&Path, File)>) -> io::Result<Launch> {
    if !available() {
        return without_bubblewrap(executable, profile, source);
    }
    let mut arguments = Arguments::default();
    arguments.base();
    arguments.add(["--clearenv", "--setenv", "MALLOC_ARENA_MAX", "2"]);
    if let Some(home) = dirs::home_dir() {
        // Libraries look for their files there; only what is bound below exists.
        arguments.setenv("HOME", home);
    }
    arguments.add(["--proc", "/proc", "--dev", "/dev", "--tmpfs", "/tmp", "--chdir", "/"]);
    if !executable.starts_with("/usr") {
        arguments.bind("--ro-bind", executable);
    }
    match profile {
        Profile::Codec => {}
        Profile::Document => arguments.reach(Reach::fonts()),
        Profile::Video | Profile::Player | Profile::Registry => {
            if let Some(registry) = registry() {
                arguments.bind(if profile == Profile::Registry { "--bind" } else { "--ro-bind" }, &registry);
                arguments.setenv("GST_REGISTRY", registry.join("registry.bin"));
                arguments.add(["--setenv", "GST_REGISTRY_UPDATE", if profile == Profile::Registry { "yes" } else { "no" }]);
            }
            if profile == Profile::Player {
                arguments.reach(Reach::sound());
            }
        }
    }
    let mut descriptors = Vec::new();
    if let Some(filter) = seccomp::filter() {
        let descriptor = seccomp::descriptor(filter)?;
        arguments.add(["--seccomp".into(), descriptor.as_raw_fd().to_string()]);
        descriptors.push(descriptor);
    }
    let source = source.map(|(path, file)| {
        let inside = inside_name(path);
        arguments.add([OsString::from("--ro-bind-fd"), file.as_raw_fd().to_string().into(), inside.clone()]);
        descriptors.push(file.into());
        inside
    });
    arguments.add(["--".into(), executable.as_os_str().to_owned()]);
    let mut command = Command::new(BWRAP);
    command.args(arguments.0);
    let inherited: Vec<RawFd> = descriptors.iter().map(AsRawFd::as_raw_fd).collect();
    // SAFETY: only fcntl between fork and exec, on descriptors this launch keeps open.
    unsafe {
        command.pre_exec(move || {
            for &descriptor in &inherited {
                rustix::io::fcntl_setfd(BorrowedFd::borrow_raw(descriptor), rustix::io::FdFlags::empty())?;
            }
            Ok(())
        });
    }
    Ok(Launch { command, source, _descriptors: descriptors })
}

/// As close to the sandbox above as Landlock allows: the same files and environment.
fn without_bubblewrap(executable: &Path, profile: Profile, source: Option<(&Path, File)>) -> io::Result<Launch> {
    let mut command = Command::new(executable);
    command.env_clear().env("MALLOC_ARENA_MAX", "2");
    if let Some(home) = dirs::home_dir() {
        command.env("HOME", home);
    }
    let mut descriptors = Vec::new();
    let ruleset = landlock::Ruleset::new();
    let allow = |path: &Path, rights| match &ruleset {
        Some(ruleset) => ruleset.allow(path, rights),
        None => Ok(()),
    };
    for name in ["/usr", "/bin", "/sbin", "/lib", "/lib32", "/lib64", "/libx32"] {
        allow(Path::new(name), landlock::RUN)?;
    }
    allow(executable, landlock::RUN)?;
    allow(Path::new("/etc/ld.so.cache"), landlock::READ)?;
    for name in ["/dev/null", "/dev/zero", "/dev/full", "/dev/random", "/dev/urandom"] {
        allow(Path::new(name), landlock::DEVICE)?;
    }
    let extra = match profile {
        Profile::Codec => Reach::default(),
        Profile::Document => Reach::fonts(),
        Profile::Video | Profile::Registry => Reach::default(),
        Profile::Player => Reach::sound(),
    };
    for path in &extra.read {
        allow(path, landlock::READ)?;
    }
    for (name, value) in extra.environment {
        command.env(name, value);
    }
    if profile.reads_media() || profile == Profile::Registry {
        if let Some(registry) = registry() {
            allow(&registry, if profile == Profile::Registry { landlock::CHANGE } else { landlock::READ })?;
            command.env("GST_REGISTRY", registry.join("registry.bin"));
            command.env("GST_REGISTRY_UPDATE", if profile == Profile::Registry { "yes" } else { "no" });
        }
    }
    let path = source.map(|(path, file)| {
        if let Some(ruleset) = &ruleset {
            ruleset.allow_file(&file, landlock::READ)?;
        }
        io::Result::Ok(path.as_os_str().to_owned())
    }).transpose()?;
    // Only the player has a sound server to reach, by a path Landlock does not cover.
    let filter = if profile == Profile::Player { seccomp::filter() } else { seccomp::filter_without_sockets() };
    let raw_ruleset = ruleset.map(|ruleset| {
        let descriptor = ruleset.into_descriptor();
        let raw = descriptor.as_raw_fd();
        descriptors.push(descriptor);
        raw
    });
    landlock::confine(&mut command, raw_ruleset, filter);
    Ok(Launch { command, source: path, _descriptors: descriptors })
}

/// Formats without a signature are recognized by their extension; the rest of the
/// user's file name stays outside.
fn inside_name(path: &Path) -> OsString {
    let mut inside = OsString::from(SOURCE);
    if let Some(extension) = path.extension().and_then(|extension| extension.to_str())
        .filter(|extension| extension.len() <= 10 && extension.bytes().all(|byte| byte.is_ascii_alphanumeric())) {
        inside.push(".");
        inside.push(extension);
    }
    inside
}

/// Oxyde's own GStreamer registry: a scan costs seconds, and a worker must not be able
/// to change what other programs load.
pub(super) fn registry() -> Option<PathBuf> {
    static REGISTRY: OnceLock<Option<PathBuf>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let directory = crate::app_paths::cache_dir()?.join("gstreamer-1.0");
        std::fs::create_dir_all(&directory)
            .inspect_err(|error| log::warn!("GStreamer registry folder unavailable: {error}")).ok()?;
        Some(directory)
    }).clone()
}

#[derive(Default)]
struct Arguments(Vec<OsString>);

impl Arguments {
    fn add<T: Into<OsString>>(&mut self, arguments: impl IntoIterator<Item = T>) {
        self.0.extend(arguments.into_iter().map(Into::into));
    }

    fn setenv(&mut self, name: &str, value: impl Into<OsString>) {
        self.add(["--setenv".into(), name.into(), value.into()]);
    }

    fn bind(&mut self, kind: &str, path: &Path) {
        self.add([OsString::from(kind), path.into(), path.into()]);
    }

    fn bind_try(&mut self, path: &Path) {
        self.bind("--ro-bind-try", path);
    }

    /// Own namespaces, without nested ones, and the system's programs and libraries.
    fn base(&mut self) {
        self.add(["--unshare-user", "--unshare-ipc", "--unshare-pid", "--unshare-net", "--unshare-uts",
            "--unshare-cgroup-try", "--disable-userns", "--new-session", "--ro-bind", "/usr", "/usr"]);
        for name in ["/bin", "/sbin", "/lib", "/lib32", "/lib64", "/libx32"] {
            match std::fs::read_link(name) {
                Ok(target) => self.add(["--symlink".into(), target.into_os_string(), name.into()]),
                Err(_) if Path::new(name).is_dir() => self.bind("--ro-bind", Path::new(name)),
                Err(_) => {}
            }
        }
        self.bind_try(Path::new("/etc/ld.so.cache"));
    }

    fn reach(&mut self, reach: Reach) {
        for path in &reach.read {
            self.bind_try(path);
        }
        for (name, value) in reach.environment {
            self.setenv(name, value);
        }
    }
}

/// Files a profile reads beyond the system, and the variables that lead libraries to them.
#[derive(Default)]
struct Reach {
    read: Vec<PathBuf>,
    environment: Vec<(&'static str, PathBuf)>,
}

impl Reach {
    fn fonts() -> Self {
        let mut reach = Self { read: vec!["/etc/fonts".into()], ..Self::default() };
        for (variable, root, folder) in [
            ("XDG_DATA_HOME", dirs::data_dir(), "fonts"),
            ("XDG_CONFIG_HOME", dirs::config_dir(), "fontconfig"),
            ("XDG_CACHE_HOME", dirs::cache_dir(), "fontconfig"),
        ] {
            if let Some(root) = root {
                reach.read.push(root.join(folder));
                reach.environment.push((variable, root));
            }
        }
        if let Some(home) = dirs::home_dir() {
            reach.read.push(home.join(".fonts"));
        }
        reach
    }

    /// PulseAudio's protocol, which PipeWire serves, then PipeWire's own.
    fn sound() -> Self {
        let mut reach = Self {
            read: ["/etc/pipewire", "/etc/pulse", "/etc/machine-id"].map(PathBuf::from).to_vec(),
            ..Self::default()
        };
        if let Some(runtime) = dirs::runtime_dir() {
            reach.read.push(runtime.join("pulse/native"));
            reach.read.push(runtime.join("pipewire-0"));
            reach.environment.push(("XDG_RUNTIME_DIR", runtime));
        }
        reach
    }
}

/// Whether bubblewrap can build the sandbox here: user namespaces may be disabled.
fn available() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        let mut arguments = Arguments::default();
        arguments.base();
        arguments.add(["--", "true"]);
        let tried = Command::new(BWRAP).args(arguments.0)
            .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped()).output();
        let refusal = match tried {
            Ok(output) if output.status.success() => return true,
            Ok(output) => String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            Err(error) => error.to_string(),
        };
        log::warn!("Preview workers run under Landlock alone: bubblewrap: {refusal}");
        false
    })
}

#[cfg(test)]
#[path = "linux_sandbox/tests/sandbox.rs"]
mod tests;
