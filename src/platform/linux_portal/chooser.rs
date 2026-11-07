//! `org.freedesktop.impl.portal.FileChooser`, as xdg-desktop-portal calls it for an
//! application. Each call names a `Request` object the backend serves until it answers,
//! so the portal can withdraw the request. Opening and saving are answered from Oxyde's
//! window; when Oxyde's interface can no longer take a call, it goes to the portal that
//! comes after Oxyde in `portals.conf`, with the same arguments, as if the portal had
//! asked it.
use crate::platform::{FileChooserChoice, FileChooserPurpose, FileChooserRequest, FileTypeFilter, FileTypePattern, InstanceMessage, InstanceRequest};
use futures::channel::{mpsc::UnboundedSender, oneshot};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Condvar, Mutex, OnceLock};
use zbus::object_server::ObjectServer;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Signature, Type, Value};

pub(in crate::platform) const PATH: &str = "/org/freedesktop/portal/desktop";
const REQUEST: &str = "org.freedesktop.impl.portal.Request";
const SUCCESS: u32 = 0;
const CANCELLED: u32 = 1;
/// Failed, or ended by the application.
const ENDED: u32 = 2;

type Options = HashMap<String, OwnedValue>;

/// Calls not answered yet. The process waits for them before ending: an application
/// would otherwise never learn the file the user chose as the last window closed.
static PENDING: (Mutex<usize>, Condvar) = (Mutex::new(0), Condvar::new());

struct Pending;

impl Pending {
    fn start() -> Self {
        *PENDING.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) += 1;
        Pending
    }
}

impl Drop for Pending {
    fn drop(&mut self) {
        let mut count = PENDING.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        *count = count.saturating_sub(1);
        PENDING.1.notify_all();
    }
}

/// Whether an application waits for a file from Oxyde, or a portal Oxyde asked for it.
pub(in crate::platform) fn answering() -> bool {
    *PENDING.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) > 0
}

pub(in crate::platform) fn wait_for_replies(limit: std::time::Duration) {
    let count = PENDING.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let (count, timeout) = PENDING.1.wait_timeout_while(count, limit, |count| *count > 0)
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if timeout.timed_out() {
        log::warn!("{} file chooser requests were still unanswered at exit", *count);
    }
}

/// A call's results, serialized as its `a{sv}`. zbus drops the reply's value once it
/// has written it to the bus, which ends the call's `Pending`.
pub(super) struct Results {
    values: Options,
    _pending: Pending,
}

impl Type for Results {
    const SIGNATURE: &'static Signature = <Options as Type>::SIGNATURE;
}

impl serde::Serialize for Results {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.values.serialize(serializer)
    }
}

fn reply(response: u32, values: Options, pending: Pending) -> (u32, Results) {
    (response, Results { values, _pending: pending })
}

fn flag(options: &Options, name: &str) -> bool {
    options.get(name).and_then(|value| bool::try_from(value).ok()).unwrap_or(false)
}

fn text(options: &Options, name: &str) -> Option<String> {
    options.get(name).and_then(|value| <&str>::try_from(value).ok()).map(str::to_owned)
}

/// A type filter as the portal writes it, `(sa(us))`: a name, then rules that are a
/// shell pattern (0) or a MIME type (1).
type RawFilter = (String, Vec<(u32, String)>);

fn raw_filter(value: &OwnedValue) -> Option<RawFilter> {
    RawFilter::try_from(value.try_clone().ok()?).ok()
}

fn raw_filters(options: &Options) -> Vec<RawFilter> {
    options.get("filters").and_then(|value| <Vec<RawFilter>>::try_from(value.try_clone().ok()?).ok()).unwrap_or_default()
}

/// The filters to offer and the one shown first: `current_filter` when the list holds
/// it; alone when the application gives no list, as the specification allows; else
/// added at the end of the list.
pub(super) fn offered_filters(mut filters: Vec<RawFilter>, current: Option<RawFilter>) -> (Vec<RawFilter>, Option<usize>) {
    let Some(current) = current else {
        let first = (!filters.is_empty()).then_some(0);
        return (filters, first);
    };
    let position = filters.iter().position(|filter| *filter == current).unwrap_or_else(|| {
        filters.push(current);
        filters.len() - 1
    });
    (filters, Some(position))
}

/// A raw filter in Oxyde's terms, its unknown rules left out.
pub(super) fn type_filter((name, rules): &RawFilter) -> FileTypeFilter {
    let patterns = rules.iter().filter_map(|(kind, pattern)| match kind {
        0 => Some(FileTypePattern::Glob(pattern.clone())),
        1 => Some(FileTypePattern::MimeType(pattern.clone())),
        _ => None,
    }).collect();
    FileTypeFilter { name: name.clone(), patterns }
}

/// A nul-terminated byte string, as the portal sends paths and names.
fn bytes(mut bytes: Vec<u8>) -> std::ffi::OsString {
    use std::os::unix::ffi::OsStringExt;
    while bytes.last() == Some(&0) {
        bytes.pop();
    }
    std::ffi::OsString::from_vec(bytes)
}

/// An absolute path option, `current_folder` or `current_file`.
fn path(options: &Options, name: &str) -> Option<PathBuf> {
    let path = PathBuf::from(bytes(<Vec<u8>>::try_from(options.get(name)?.try_clone().ok()?).ok()?));
    path.is_absolute().then_some(path)
}

/// The folder to open first: that of `current_file`, the file saved again, else
/// `current_folder`, when it is a folder.
fn suggested_folder(options: &Options) -> Option<PathBuf> {
    let saved = path(options, "current_file").and_then(|file| file.parent().map(Path::to_path_buf));
    saved.into_iter().chain(path(options, "current_folder")).find(|folder| folder.is_dir())
}

/// What the call asks for: the name to save under is `current_file`'s, else
/// `current_name`; files saved together keep only their names' last component.
fn purpose(method: &str, options: &Options) -> FileChooserPurpose {
    match method {
        "SaveFile" => {
            let saved = path(options, "current_file").and_then(|file| file.file_name().map(|name| name.to_string_lossy().into_owned()));
            FileChooserPurpose::Save { name: saved.or_else(|| text(options, "current_name")).unwrap_or_default() }
        }
        "SaveFiles" => {
            let names = options.get("files").and_then(|value| <Vec<Vec<u8>>>::try_from(value.try_clone().ok()?).ok()).unwrap_or_default();
            FileChooserPurpose::SaveFiles {
                names: names.into_iter().filter_map(|name| Path::new(&bytes(name)).file_name().map(ToOwned::to_owned)).collect(),
            }
        }
        _ => FileChooserPurpose::Open,
    }
}

/// A choice as the portal writes it, `(ssa(ss)s)`: an id, a label, the answers offered
/// by id and label, none for a checkbox, and the one given first.
type RawChoice = (String, String, Vec<(String, String)>, String);

fn raw_choices(options: &Options) -> Vec<RawChoice> {
    options.get("choices").and_then(|value| <Vec<RawChoice>>::try_from(value.try_clone().ok()?).ok()).unwrap_or_default()
}

/// A raw choice in Oxyde's terms. An answer given first that is not offered becomes
/// the first one, or "false" for a checkbox.
pub(super) fn choice((id, label, options, initial): RawChoice) -> FileChooserChoice {
    let initial = match options.first() {
        None if initial == "true" => initial,
        None => "false".into(),
        Some(_) if options.iter().any(|(option, _)| *option == initial) => initial,
        Some((first, _)) => first.clone(),
    };
    let options = options.into_iter().map(|(id, label)| (id, without_mnemonics(&label))).collect();
    FileChooserChoice { id, label: without_mnemonics(&label), options, initial }
}

/// A GTK label without its mnemonic marks: `_Open` reads `Open`, `__` an underscore.
pub(super) fn without_mnemonics(label: &str) -> String {
    let mut plain = String::with_capacity(label.len());
    let mut characters = label.chars().peekable();
    while let Some(character) = characters.next() {
        if character != '_' {
            plain.push(character);
        } else if characters.peek() == Some(&'_') {
            plain.push('_');
            characters.next();
        }
    }
    plain
}

/// Bytes a path keeps in a URI, as GLib's `g_filename_to_uri` keeps them.
const PATH_BYTES: &percent_encoding::AsciiSet = &percent_encoding::NON_ALPHANUMERIC
    .remove(b'-').remove(b'.').remove(b'_').remove(b'~').remove(b'/')
    .remove(b'!').remove(b'$').remove(b'&').remove(b'\'').remove(b'(').remove(b')')
    .remove(b'*').remove(b'+').remove(b',').remove(b';').remove(b'=').remove(b':').remove(b'@');

/// The `file://` URI of a local path, its bytes kept whatever their encoding.
pub(super) fn uri(path: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;
    format!("file://{}", percent_encoding::percent_encode(path.as_os_str().as_bytes(), PATH_BYTES))
}

/// The portal's call ended by the application, as the call that waits sees it.
enum Withdrawal {
    /// Oxyde's own window, which closes.
    Window(Mutex<Option<oneshot::Sender<()>>>),
    /// The fallback portal's request, at the same path on that portal.
    Forwarded(String, OwnedObjectPath),
}

struct Request(Withdrawal);

#[zbus::interface(name = "org.freedesktop.impl.portal.Request")]
impl Request {
    async fn close(&self) {
        match &self.0 {
            Withdrawal::Window(sender) => {
                if let Some(sender) = sender.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).take() {
                    let _ = sender.send(());
                }
            }
            Withdrawal::Forwarded(backend, path) => {
                let result = match forwarding().await {
                    Ok(connection) => connection.call_method(Some(backend.as_str()), path, Some(REQUEST), "Close", &()).await.map(drop),
                    Err(error) => Err(error),
                };
                if let Err(error) = result {
                    log::warn!("Cannot withdraw the request from {backend}: {error}");
                }
            }
        }
    }
}

/// A connection without Oxyde's own five-second limit: the fallback portal answers only
/// once the user has chosen, however long that takes.
async fn forwarding() -> zbus::Result<&'static zbus::Connection> {
    static CONNECTION: OnceLock<zbus::Connection> = OnceLock::new();
    if let Some(connection) = CONNECTION.get() {
        return Ok(connection);
    }
    let connection = zbus::Connection::session().await?;
    Ok(CONNECTION.get_or_init(|| connection))
}

/// The call's arguments, as the portal sent them.
struct Call {
    handle: OwnedObjectPath,
    app_id: String,
    parent_window: String,
    title: String,
    options: Options,
}

/// Hands the call to the portal after Oxyde's, which answers it.
async fn forward(method: &str, call: Call, server: &ObjectServer, pending: Pending) -> (u32, Results) {
    let Some(backend) = super::fallback() else {
        log::warn!("No portal after Oxyde chooses files for {method}");
        return reply(ENDED, Options::new(), pending);
    };
    let connection = match forwarding().await {
        Ok(connection) => connection,
        Err(error) => {
            log::warn!("Cannot reach {backend} for {method}: {error}");
            return reply(ENDED, Options::new(), pending);
        }
    };
    let handle = call.handle.clone();
    if let Err(error) = server.at(&handle, Request(Withdrawal::Forwarded(backend.clone(), handle.clone()))).await {
        log::warn!("Cannot serve the request of {method}: {error}");
    }
    let arguments = (&call.handle, &call.app_id, &call.parent_window, &call.title, &call.options);
    let answer = connection.call_method(Some(backend.as_str()), PATH, Some(super::INTERFACE), method, &arguments).await
        .and_then(|message| Ok(message.body().deserialize::<(u32, Options)>()?));
    let _ = server.remove::<Request, _>(&handle).await;
    match answer {
        Ok((response, values)) => reply(response, values, pending),
        Err(error) => {
            log::warn!("{backend} did not answer {method}: {error}");
            reply(ENDED, Options::new(), pending)
        }
    }
}

pub(in crate::platform) struct FileChooser(pub(in crate::platform) UnboundedSender<InstanceMessage>);

impl FileChooser {
    /// Asks Oxyde's interface for a window that answers `method`.
    async fn open_in_window(&self, method: &str, call: Call, server: &ObjectServer, pending: Pending) -> (u32, Results) {
        let (answer, answered) = oneshot::channel();
        let (withdraw, withdrawn) = oneshot::channel();
        if let Err(error) = server.at(&call.handle, Request(Withdrawal::Window(Mutex::new(Some(withdraw))))).await {
            log::warn!("Cannot serve the request of {method}: {error}");
        }
        log::info!("{method} for {:?}", call.app_id);
        let options = &call.options;
        let opening = method == "OpenFile";
        // Saving several files chooses their folder, without type filters.
        let (filters, current_filter) = if method == "SaveFiles" { (Vec::new(), None) }
            else { offered_filters(raw_filters(options), options.get("current_filter").and_then(raw_filter)) };
        let request = FileChooserRequest {
            purpose: purpose(method, options),
            title: call.title.clone(),
            accept_label: text(options, "accept_label").map(|label| without_mnemonics(&label)),
            multiple: opening && flag(options, "multiple"),
            directory: method == "SaveFiles" || (opening && flag(options, "directory")),
            filters: filters.iter().map(type_filter).collect(),
            current_filter,
            current_folder: suggested_folder(options),
            choices: raw_choices(options).into_iter().map(choice).collect(),
            answer,
            withdrawn,
        };
        let asked = !request.choices.is_empty();
        let message = InstanceMessage { request: InstanceRequest::ChooseFile(request), activation_token: None };
        if self.0.unbounded_send(message).is_err() {
            // Oxyde's interface has gone: the portal after it answers instead.
            let _ = server.remove::<Request, _>(&call.handle).await;
            return forward(method, call, server, pending).await;
        }
        let answer = answered.await;
        let _ = server.remove::<Request, _>(&call.handle).await;
        match answer {
            Ok(Some(chosen)) => {
                let uris: Vec<String> = chosen.paths.iter().map(|path| uri(path)).collect();
                let mut values = Options::new();
                if let Ok(uris) = OwnedValue::try_from(Value::from(uris)) {
                    values.insert("uris".into(), uris);
                }
                // The filter shown, which the application may keep for next time.
                let shown = chosen.filter.and_then(|position| filters.get(position)).cloned();
                if let Some(filter) = shown.and_then(|filter| OwnedValue::try_from(Value::from(filter)).ok()) {
                    values.insert("current_filter".into(), filter);
                }
                if let Some(choices) = asked.then(|| OwnedValue::try_from(Value::from(chosen.choices)).ok()).flatten() {
                    values.insert("choices".into(), choices);
                }
                reply(SUCCESS, values, pending)
            }
            Ok(None) => reply(CANCELLED, Options::new(), pending),
            // The window closed without an answer: withdrawn, or Oxyde is ending.
            Err(oneshot::Canceled) => reply(ENDED, Options::new(), pending),
        }
    }
}

#[zbus::interface(name = "org.freedesktop.impl.portal.FileChooser")]
impl FileChooser {
    #[zbus(out_args("response", "results"))]
    async fn open_file(
        &self,
        handle: OwnedObjectPath,
        app_id: String,
        parent_window: String,
        title: String,
        options: Options,
        #[zbus(object_server)] server: &ObjectServer,
    ) -> (u32, Results) {
        let pending = Pending::start();
        self.open_in_window("OpenFile", Call { handle, app_id, parent_window, title, options }, server, pending).await
    }

    #[zbus(out_args("response", "results"))]
    async fn save_file(
        &self,
        handle: OwnedObjectPath,
        app_id: String,
        parent_window: String,
        title: String,
        options: Options,
        #[zbus(object_server)] server: &ObjectServer,
    ) -> (u32, Results) {
        let pending = Pending::start();
        self.open_in_window("SaveFile", Call { handle, app_id, parent_window, title, options }, server, pending).await
    }

    #[zbus(out_args("response", "results"))]
    async fn save_files(
        &self,
        handle: OwnedObjectPath,
        app_id: String,
        parent_window: String,
        title: String,
        options: Options,
        #[zbus(object_server)] server: &ObjectServer,
    ) -> (u32, Results) {
        let pending = Pending::start();
        self.open_in_window("SaveFiles", Call { handle, app_id, parent_window, title, options }, server, pending).await
    }
}

#[cfg(test)]
#[path = "tests/chooser.rs"]
mod tests;
