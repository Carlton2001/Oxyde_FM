//! Processes that keep a volume busy, as GNOME's "Volume is busy" dialog lists them.
//! Only the user's own processes are readable; others stay unnamed.
use std::collections::BTreeSet;
use std::path::Path;

/// Names of the processes with a working directory, root, executable, open file or
/// mapping under one of `points`.
pub(super) fn holders(points: &[String]) -> Vec<String> {
    let under = |path: &Path| {
        points.iter().any(|point| path.starts_with(point))
    };
    let Ok(processes) = std::fs::read_dir("/proc") else { return Vec::new() };
    let names: BTreeSet<String> = processes
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy().bytes().all(|byte| byte.is_ascii_digit()))
        .map(|entry| entry.path())
        .filter(|process| holds(process, &under))
        .filter_map(|process| name(&process))
        .collect();
    names.into_iter().collect()
}

fn holds(process: &Path, under: &impl Fn(&Path) -> bool) -> bool {
    let links = ["cwd", "root", "exe"].into_iter().filter_map(|link| std::fs::read_link(process.join(link)).ok());
    let files = std::fs::read_dir(process.join("fd"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|fd| std::fs::read_link(fd.path()).ok());
    if links.chain(files).any(|path| under(&path)) {
        return true;
    }
    // The path is the sixth column; mapped libraries and fonts count too.
    std::fs::read_to_string(process.join("maps")).is_ok_and(|maps| {
        maps.lines().filter_map(|line| line.split_whitespace().nth(5)).any(|path| under(Path::new(path)))
    })
}

/// `comm` is cut at 15 bytes; the executable then gives the full name.
fn name(process: &Path) -> Option<String> {
    let comm = std::fs::read_to_string(process.join("comm")).ok()?.trim_end().to_string();
    if comm.len() < 15 {
        return Some(comm);
    }
    std::fs::read_link(process.join("exe"))
        .ok()
        .and_then(|exe| exe.file_name().map(|name| name.to_string_lossy().into_owned()))
        .filter(|name| name.starts_with(&comm))
        .or(Some(comm))
}

#[cfg(test)]
#[path = "tests/holders.rs"]
mod tests;
