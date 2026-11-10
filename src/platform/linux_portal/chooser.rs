//! `org.freedesktop.impl.portal.FileChooser`, as xdg-desktop-portal calls it for an
//! application. Each call names a `Request` object the backend serves until it answers,
//! so the portal can withdraw the request. Opening and saving are answered from Oxyde's
//! window; when Oxyde's interface cannot show one, in time or at all, the call goes to
//! the portal that comes after Oxyde in `portals.conf`, with the same arguments, as if
//! the portal had asked it. The portal waits for its backends without a limit and
//! applies its own fallback only to a backend that is missing: a failing Oxyde would
//! otherwise leave the application without a file chooser.
use super::pending::{self, Pending};
use crate::platform::{FileChooserAnswer, FileChooserChoice, FileChooserParent, FileChooserPurpose, FileChooserRequest, FileTypeFilter, FileTypePattern, InstanceMessage, InstanceRequest};
use futures::channel::{mpsc::UnboundedSender, oneshot};
use futures::{FutureExt, StreamExt};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use zbus::object_server::ObjectServer;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Signature, Type, Value};

pub(in crate::platform) const PATH: &str = "/org/freedesktop/portal/desktop";
const SUCCESS: u32 = 0;
const CANCELLED: u32 = 1;
/// Failed, or ended by the application.
pub(super) const ENDED: u32 = 2;

pub(super) type Options = HashMap<String, OwnedValue>;

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

/// The application's window, from the portal's `x11:<hex id>` or `wayland:<handle>`;
/// nothing for an empty or unknown form.
pub(super) fn parent_window(text: &str) -> Option<FileChooserParent> {
    if let Some(id) = text.strip_prefix("x11:") {
        let digits = id.strip_prefix("0x").or_else(|| id.strip_prefix("0X")).unwrap_or(id);
        return u32::from_str_radix(digits, 16).ok().filter(|&id| id != 0).map(FileChooserParent::X11);
    }
    text.strip_prefix("wayland:").filter(|handle| !handle.is_empty()).map(|handle| FileChooserParent::Wayland(handle.to_owned()))
}

/// The `file://` URI of a local path, its bytes kept whatever their encoding.
pub(super) fn uri(path: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;
    format!("file://{}", percent_encoding::percent_encode(path.as_os_str().as_bytes(), PATH_BYTES))
}

/// How long Oxyde's interface may take to show its window, a start for the call
/// included, before the portal after Oxyde's answers instead.
const SHOWING_LIMIT: Duration = Duration::from_secs(10);

/// Closes Oxyde's window unanswered, once.
struct Withdraw(Mutex<Option<oneshot::Sender<()>>>);

impl Withdraw {
    fn new(sender: oneshot::Sender<()>) -> Arc<Self> {
        Arc::new(Self(Mutex::new(Some(sender))))
    }

    /// Whether the window was still to answer: false when already withdrawn.
    fn withdraw(&self) -> bool {
        let sender = self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).take();
        sender.map(|sender| { let _ = sender.send(()); }).is_some()
    }
}

/// The portal's call ended by the application, as the call that waits sees it.
enum Withdrawal {
    /// Oxyde's own window, which closes.
    Window(Arc<Withdraw>),
    /// The fallback portal's request, at the same path on that portal.
    Forwarded(String, OwnedObjectPath),
}

struct Request(Withdrawal);

#[zbus::interface(name = "org.freedesktop.impl.portal.Request")]
impl Request {
    async fn close(&self) {
        match &self.0 {
            Withdrawal::Window(withdraw) => {
                withdraw.withdraw();
            }
            Withdrawal::Forwarded(backend, path) => super::forward::close(backend, path).await,
        }
    }
}

/// The call's arguments, as the portal sent them, and who sent it.
pub(super) struct Call {
    pub(super) handle: OwnedObjectPath,
    pub(super) app_id: String,
    pub(super) parent_window: String,
    pub(super) title: String,
    pub(super) options: Options,
    /// The portal's unique name on the bus, which the call needs alive.
    caller: Option<String>,
}

impl Call {
    fn new(header: &zbus::message::Header<'_>, handle: OwnedObjectPath, app_id: String, parent_window: String, title: String, options: Options) -> Self {
        let caller = header.sender().map(|sender| sender.to_string());
        Self { handle, app_id, parent_window, title, options, caller }
    }

    /// Resolves once the portal that called has left the bus, as when it restarts:
    /// nobody waits for the answer any more. Never when that cannot be watched.
    pub(super) async fn abandoned(&self, connection: &zbus::Connection) {
        let Some(caller) = self.caller.as_deref() else { return futures::future::pending().await };
        let watched = async {
            let bus = zbus::fdo::DBusProxy::builder(connection).cache_properties(zbus::proxy::CacheProperties::No).build().await?;
            let mut changes = bus.receive_name_owner_changed_with_args(&[(0, caller)]).await?;
            // Subscribed first, so that a departure in between is not missed.
            if !bus.name_has_owner(zbus::names::BusName::try_from(caller)?).await? {
                return Ok(true);
            }
            while let Some(change) = changes.next().await {
                if change.args()?.new_owner().is_none() {
                    return Ok(true);
                }
            }
            Ok::<_, zbus::Error>(false)
        };
        match watched.await {
            Ok(true) => log::info!("{caller} left the bus while waiting for a file"),
            Ok(false) => futures::future::pending().await,
            Err(error) => {
                log::warn!("Cannot watch {caller} while it waits for a file: {error}");
                futures::future::pending().await
            }
        }
    }
}

/// Hands the call to the portal after Oxyde's, which answers it.
async fn forward(method: &str, call: &Call, server: &ObjectServer, connection: &zbus::Connection) -> (u32, Options) {
    let Some(backend) = super::fallback() else {
        log::warn!("No portal after Oxyde chooses files for {method}");
        return (ENDED, Options::new());
    };
    let handle = &call.handle;
    if let Err(error) = server.at(handle, Request(Withdrawal::Forwarded(backend.clone(), handle.clone()))).await {
        log::warn!("Cannot serve the request of {method}: {error}");
    }
    let answer = super::forward::ask(&backend, method, call, connection).await;
    let _ = server.remove::<Request, _>(handle).await;
    answer
}

/// How a call to Oxyde's window ended.
enum Outcome {
    Answered(Option<FileChooserAnswer>),
    /// The window closed unanswered: withdrawn, or Oxyde is ending.
    Closed,
    /// No window, in time or at all, or one the interface can no longer answer from.
    Unserved(&'static str),
    /// The portal that called has gone.
    Abandoned,
}

/// Waits for the window's answer, its showing in time and the portal's presence.
async fn outcome(shown: oneshot::Receiver<()>, answered: oneshot::Receiver<Option<FileChooserAnswer>>, abandoned: impl std::future::Future<Output = ()>) -> Outcome {
    let mut shown = shown.fuse();
    let mut answered = answered.fuse();
    let mut late = FutureExt::fuse(async_io::Timer::after(SHOWING_LIMIT));
    let mut failed = pending::interface_failure().fuse();
    let mut abandoned = std::pin::pin!(abandoned.fuse());
    let mut showing = true;
    loop {
        futures::select_biased! {
            _ = failed => return Outcome::Unserved("Oxyde's interface failed"),
            () = abandoned => return Outcome::Abandoned,
            result = shown => match result {
                Ok(()) => {
                    showing = false;
                    super::guard::cleared();
                }
                Err(oneshot::Canceled) => return Outcome::Unserved("Oxyde could not show its window"),
            },
            answer = answered => return answer.map_or(Outcome::Closed, Outcome::Answered),
            _ = late => if showing {
                return Outcome::Unserved("Oxyde's window did not show in time");
            },
        }
    }
}

/// The values a choice answers with.
fn chosen_values(chosen: FileChooserAnswer, filters: &[RawFilter], asked: bool) -> Options {
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
    values
}

pub(in crate::platform) struct FileChooser(pub(in crate::platform) UnboundedSender<InstanceMessage>);

impl FileChooser {
    /// Asks Oxyde's interface for a window that answers `method`.
    async fn open_in_window(&self, method: &str, call: Call, server: &ObjectServer, connection: &zbus::Connection) -> (u32, Results) {
        let mut pending = Pending::start();
        let (answer, answered) = oneshot::channel();
        let (withdraw, withdrawn) = oneshot::channel();
        let (shown, showing) = oneshot::channel();
        let withdraw = Withdraw::new(withdraw);
        if let Err(error) = server.at(&call.handle, Request(Withdrawal::Window(withdraw.clone()))).await {
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
            default_extension: None,
            choices: raw_choices(options).into_iter().map(choice).collect(),
            parent: parent_window(&call.parent_window),
            answer,
            withdrawn,
            shown: Some(shown),
        };
        let asked = !request.choices.is_empty();
        let message = InstanceMessage { request: InstanceRequest::ChooseFile(request), activation_token: None };
        let outcome = if self.0.unbounded_send(message).is_err() {
            Outcome::Unserved("Oxyde's interface has gone")
        } else {
            outcome(showing, answered, call.abandoned(connection)).await
        };
        let _ = server.remove::<Request, _>(&call.handle).await;
        match outcome {
            Outcome::Answered(Some(chosen)) => reply(SUCCESS, chosen_values(chosen, &filters, asked), pending),
            Outcome::Answered(None) => reply(CANCELLED, Options::new(), pending),
            Outcome::Closed => reply(ENDED, Options::new(), pending),
            Outcome::Abandoned => {
                withdraw.withdraw();
                reply(ENDED, Options::new(), pending)
            }
            // Withdrawn meanwhile: nothing to hand on. Otherwise a window that would
            // still come closes, and the portal after Oxyde's answers.
            Outcome::Unserved(_) if !withdraw.withdraw() => reply(ENDED, Options::new(), pending),
            Outcome::Unserved(reason) => {
                log::warn!("{reason}: {method} goes to the portal after Oxyde's");
                pending.forward();
                let (response, values) = forward(method, &call, server, connection).await;
                reply(response, values, pending)
            }
        }
    }
}

#[zbus::interface(name = "org.freedesktop.impl.portal.FileChooser")]
impl FileChooser {
    #[zbus(out_args("response", "results"))]
    #[allow(clippy::too_many_arguments)]
    async fn open_file(
        &self,
        handle: OwnedObjectPath,
        app_id: String,
        parent_window: String,
        title: String,
        options: Options,
        #[zbus(header)] header: zbus::message::Header<'_>,
        #[zbus(object_server)] server: &ObjectServer,
        #[zbus(connection)] connection: &zbus::Connection,
    ) -> (u32, Results) {
        let call = Call::new(&header, handle, app_id, parent_window, title, options);
        self.open_in_window("OpenFile", call, server, connection).await
    }

    #[zbus(out_args("response", "results"))]
    #[allow(clippy::too_many_arguments)]
    async fn save_file(
        &self,
        handle: OwnedObjectPath,
        app_id: String,
        parent_window: String,
        title: String,
        options: Options,
        #[zbus(header)] header: zbus::message::Header<'_>,
        #[zbus(object_server)] server: &ObjectServer,
        #[zbus(connection)] connection: &zbus::Connection,
    ) -> (u32, Results) {
        let call = Call::new(&header, handle, app_id, parent_window, title, options);
        self.open_in_window("SaveFile", call, server, connection).await
    }

    #[zbus(out_args("response", "results"))]
    #[allow(clippy::too_many_arguments)]
    async fn save_files(
        &self,
        handle: OwnedObjectPath,
        app_id: String,
        parent_window: String,
        title: String,
        options: Options,
        #[zbus(header)] header: zbus::message::Header<'_>,
        #[zbus(object_server)] server: &ObjectServer,
        #[zbus(connection)] connection: &zbus::Connection,
    ) -> (u32, Results) {
        let call = Call::new(&header, handle, app_id, parent_window, title, options);
        self.open_in_window("SaveFiles", call, server, connection).await
    }
}

#[cfg(test)]
#[path = "tests/chooser.rs"]
mod tests;
