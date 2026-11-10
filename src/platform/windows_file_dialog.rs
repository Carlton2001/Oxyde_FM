//! Open and Save dialogs of other applications, answered by Oxyde's file chooser
//! (experimental). A small library, registered for the user in place of the system's
//! dialogs, is loaded by the application that asks; it holds no protocol and runs the
//! launcher with what was asked (`relay`), which hands it to the running Oxyde over the
//! instance pipe, or to one it starts. Oxyde answers on a pipe the launcher made for
//! this dialog: first that its window is shown, then what the user chose. Whenever
//! that fails, the launcher says so and the library shows the system's dialog.
//!
//! What is asked is text, a line for each `key<TAB>value`:
//! `purpose` (`open`, `save` or `folder`), `multiple`, `title`, `accept`, `name`,
//! `folder`, `extension`, `owner` (the application's window), `filter` (a name, a tab
//! and its patterns, as often as there are filters) and `filter-index`, counted from 1
//! as Windows does. The answer is `cancel`, or `ok` followed by `filter<TAB>index` and
//! a `path<TAB>…` line for each item chosen.
pub(super) mod registration;
pub(super) mod relay;

use super::{FileChooserAnswer, FileChooserParent, FileChooserPurpose, FileChooserRequest, FileTypeFilter, FileTypePattern};
use futures::channel::oneshot;
use std::io::Write;
use std::os::windows::io::AsRawHandle;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Threading::{OpenProcess, WaitForSingleObject, INFINITE, PROCESS_SYNCHRONIZE};
use windows::Win32::UI::WindowsAndMessaging::{AllowSetForegroundWindow, ASFW_ANY};

/// What Oxyde writes once the window is open; the answer follows.
const SHOWN: &str = "shown\n";

/// What an application asks its dialog for, as the library wrote it.
#[derive(Debug, Default, PartialEq)]
struct Asked {
    purpose: String,
    multiple: bool,
    title: String,
    accept: Option<String>,
    name: String,
    folder: Option<PathBuf>,
    extension: Option<String>,
    owner: Option<isize>,
    filters: Vec<FileTypeFilter>,
    /// The filter shown first, counted from 1.
    filter: Option<usize>,
}

/// The patterns of a filter, written `*.png;*.jpg`. Windows reads `*.*` as every
/// file, with or without an extension.
fn patterns(specification: &str) -> Vec<FileTypePattern> {
    specification.split(';')
        .map(str::trim)
        .filter(|pattern| !pattern.is_empty())
        .map(|pattern| FileTypePattern::Glob(if pattern == "*.*" { "*".into() } else { pattern.to_owned() }))
        .collect()
}

fn asked(lines: &[String]) -> Asked {
    let mut asked = Asked::default();
    for line in lines {
        let Some((key, value)) = line.split_once('\t') else { continue };
        match key {
            "purpose" => asked.purpose = value.to_owned(),
            "multiple" => asked.multiple = value == "1",
            "title" => asked.title = value.to_owned(),
            "accept" => asked.accept = Some(value.replace('&', "")).filter(|label| !label.is_empty()),
            "name" => asked.name = value.to_owned(),
            "folder" => asked.folder = Some(PathBuf::from(value)).filter(|folder| folder.is_absolute()),
            "extension" => asked.extension = Some(value.trim_start_matches('.').to_owned()).filter(|extension| !extension.is_empty()),
            "owner" => asked.owner = value.parse().ok().filter(|owner| *owner != 0),
            "filter-index" => asked.filter = value.parse().ok(),
            "filter" => {
                let (name, specification) = value.split_once('\t').unwrap_or((value, ""));
                asked.filters.push(FileTypeFilter { name: name.to_owned(), patterns: patterns(specification) });
            }
            _ => {}
        }
    }
    asked
}

/// The answer, as the library reads it.
fn answer_text(answer: Option<&FileChooserAnswer>) -> String {
    let Some(answer) = answer else { return "cancel\n".into() };
    let mut text = String::from("ok\n");
    if let Some(filter) = answer.filter {
        text.push_str(&format!("filter\t{}\n", filter + 1));
    }
    for path in answer.paths.iter().filter_map(|path| path.to_str()).filter(|path| !path.contains(['\r', '\n'])) {
        text.push_str(&format!("path\t{path}\n"));
    }
    text
}

/// Dialogs whose application still waits for Oxyde's answer.
static PENDING: AtomicUsize = AtomicUsize::new(0);

pub(super) fn answering() -> bool {
    PENDING.load(Ordering::SeqCst) > 0
}

/// Waits for the answers on their way to be written, as Oxyde ends: a moment at most.
pub(super) fn finish_answers() {
    let started = std::time::Instant::now();
    while answering() && started.elapsed() < std::time::Duration::from_secs(2) {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

struct Pending;
impl Pending {
    fn start() -> Self {
        PENDING.fetch_add(1, Ordering::SeqCst);
        Self
    }
}
impl Drop for Pending {
    fn drop(&mut self) { PENDING.fetch_sub(1, Ordering::SeqCst); }
}

/// The launcher's pipe for this dialog, once it is known to be the user's.
fn reply_pipe(name: &str) -> std::io::Result<std::fs::File> {
    let pipe = std::fs::OpenOptions::new().write(true).open(name)?;
    if !super::windows_instance::owned_by_this_user(HANDLE(pipe.as_raw_handle())) {
        return Err(std::io::Error::other("the pipe belongs to another account"));
    }
    Ok(pipe)
}

type Withdraw = Arc<Mutex<Option<oneshot::Sender<()>>>>;

/// Tells the launcher the window is shown, then what the user chose. A window that
/// never opens, or closes unanswered, closes the pipe: the system's dialog is shown.
fn reply(name: String, showing: oneshot::Receiver<()>, answered: oneshot::Receiver<Option<FileChooserAnswer>>, _withdraw: Withdraw, _pending: Pending) {
    if futures::executor::block_on(showing).is_err() {
        return;
    }
    let written = reply_pipe(&name).and_then(|mut pipe| {
        pipe.write_all(SHOWN.as_bytes())?;
        let Ok(answer) = futures::executor::block_on(answered) else { return Ok(()) };
        // The application takes the foreground back from the window that closes.
        unsafe { let _ = AllowSetForegroundWindow(ASFW_ANY); }
        pipe.write_all(answer_text(answer.as_ref()).as_bytes())?;
        pipe.flush()
    });
    if let Err(error) = written {
        log::warn!("Cannot answer an application's dialog: {error}");
    }
}

/// Withdraws the request when the launcher ends before its answer: the application
/// closed its dialog, or gave Oxyde up for the system's.
fn watch(launcher: u32, withdraw: Withdraw) {
    unsafe {
        let Ok(process) = OpenProcess(PROCESS_SYNCHRONIZE, false, launcher) else { return };
        WaitForSingleObject(process, INFINITE);
        let _ = CloseHandle(process);
    }
    if let Some(withdraw) = withdraw.lock().ok().and_then(|mut withdraw| withdraw.take()) {
        let _ = withdraw.send(());
    }
}

/// The request of the dialog a launcher handed over: the pipe to answer on, then what
/// is asked. None when it names no pipe.
pub(super) fn request(lines: &[String], launcher: Option<u32>) -> Option<FileChooserRequest> {
    let (pipe, lines) = lines.split_first()?;
    if !pipe.starts_with(relay::PIPES) {
        return None;
    }
    let asked = asked(lines);
    let (answer, answered) = oneshot::channel();
    let (withdraw, withdrawn) = oneshot::channel();
    let (shown, showing) = oneshot::channel();
    let withdraw: Withdraw = Arc::new(Mutex::new(Some(withdraw)));
    let pending = Pending::start();
    let (name, kept) = (pipe.clone(), withdraw.clone());
    std::thread::Builder::new().name("oxyde-dialog-answer".into()).spawn(move || reply(name, showing, answered, kept, pending)).ok()?;
    if let Some(launcher) = launcher {
        let _ = std::thread::Builder::new().name("oxyde-dialog-watch".into()).spawn(move || watch(launcher, withdraw));
    }
    let folders = asked.purpose == "folder";
    let current_filter = asked.filter.and_then(|filter| filter.checked_sub(1)).filter(|filter| *filter < asked.filters.len())
        .or((!asked.filters.is_empty()).then_some(0));
    Some(FileChooserRequest {
        purpose: if asked.purpose == "save" { FileChooserPurpose::Save { name: asked.name } } else { FileChooserPurpose::Open },
        title: asked.title,
        accept_label: asked.accept,
        multiple: asked.multiple && asked.purpose != "save",
        directory: folders,
        filters: if folders { Vec::new() } else { asked.filters },
        current_filter: current_filter.filter(|_| !folders),
        // Read here, off the interface thread: a folder that is gone leaves the last one.
        current_folder: asked.folder.filter(|folder| folder.is_dir()),
        default_extension: asked.extension,
        choices: Vec::new(),
        parent: asked.owner.map(FileChooserParent::Windows),
        answer,
        withdrawn,
        shown: Some(shown),
    })
}

#[cfg(test)]
#[path = "windows_file_dialog/tests/requests.rs"]
mod tests;
