//! Opens a folder in a terminal. Linux has no default terminal yet: `xdg-terminal-exec`
//! is the coming standard, and otherwise the desktop's own terminal goes first.
use std::path::Path;
use std::process::{Command, Stdio};

/// A terminal and the arguments before the folder. Terminals that hand over to a running
/// instance ignore their working directory and need the folder as an argument; an empty
/// list stands for those that start in their working directory.
type Terminal = (&'static str, &'static [&'static str]);

const XDG_TERMINAL_EXEC: Terminal = ("xdg-terminal-exec", &[]);
const PTYXIS: Terminal = ("ptyxis", &["--new-window", "--working-directory"]);
const GNOME_TERMINAL: Terminal = ("gnome-terminal", &["--working-directory"]);
const GNOME_CONSOLE: Terminal = ("kgx", &["--working-directory"]);
const KONSOLE: Terminal = ("konsole", &["--workdir"]);
const XFCE_TERMINAL: Terminal = ("xfce4-terminal", &["--working-directory"]);
const MATE_TERMINAL: Terminal = ("mate-terminal", &["--working-directory"]);
const OTHERS: &[Terminal] = &[
    ("tilix", &["--working-directory"]),
    ("alacritty", &["--working-directory"]),
    ("kitty", &["--directory"]),
    ("foot", &["--working-directory"]),
    ("xterm", &[]),
];

/// The terminals to try under `desktop` (`XDG_CURRENT_DESKTOP`), best first.
fn terminals(desktop: &str) -> Vec<Terminal> {
    let on = |name: &str| desktop.split(':').any(|part| part.eq_ignore_ascii_case(name));
    let mut own = Vec::new();
    if on("KDE") { own.push(KONSOLE); }
    if on("XFCE") { own.push(XFCE_TERMINAL); }
    if on("MATE") { own.push(MATE_TERMINAL); }
    let mut list = vec![XDG_TERMINAL_EXEC];
    list.extend(own.iter().copied());
    for terminal in [PTYXIS, GNOME_CONSOLE, GNOME_TERMINAL, KONSOLE, XFCE_TERMINAL, MATE_TERMINAL] {
        if !own.contains(&terminal) { list.push(terminal); }
    }
    list.extend_from_slice(OTHERS);
    list
}

/// Starts the first terminal found, then waits for it in the background so that it does
/// not stay a zombie.
pub(super) fn open(path: &Path) -> Result<(), String> {
    // A missing working directory would pass for a missing terminal.
    std::fs::read_dir(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    for (program, arguments) in terminals(&desktop) {
        let mut command = Command::new(program);
        super::linux_gpu::leave_out_of(&mut command);
        command.current_dir(path).args(arguments);
        if !arguments.is_empty() { command.arg(path); }
        command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        match command.spawn() {
            Ok(mut child) => {
                std::thread::spawn(move || child.wait());
                return Ok(());
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("{program}: {error}")),
        }
    }
    Err("No terminal was found".into())
}

#[cfg(test)]
mod tests;
