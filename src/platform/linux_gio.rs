//! GVfs through GIO, as Nautilus uses it: mounting with credentials, enumerating the
//! network and the shares of a server, and mounted shares at their local FUSE path.
use super::remote::{PasswordSave, RemoteAnswer, RemoteError, RemoteLocation, RemotePrompt};
use super::VolumeError;
use library::{gio, GError, Gio, Object};
use std::ffi::{c_char, c_uint, c_void, CStr, CString};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

mod library;

// GIOErrorEnum.
const ALREADY_MOUNTED: i32 = 17;
const CANCELLED: i32 = 19;
const BUSY: i32 = 26;
const FAILED_HANDLED: i32 = 30;
// GFileType.
const DIRECTORY: c_uint = 2;
const SHORTCUT: c_uint = 5;
const MOUNTABLE: c_uint = 6;

/// Schemes GVfs mounts that Oxyde opens, `network` being the neighborhood.
const SCHEMES: &[&str] = &["smb", "sftp", "ftp", "ftps", "dav", "davs", "afp", "nfs", "network"];
/// Mount types under the FUSE folder that the drive list shows as network shares.
const NETWORK_MOUNTS: &[&str] = &["smb-share", "sftp", "ftp", "ftps", "dav", "davs", "afp-volume", "nfs"];

pub(super) fn is_remote_uri(text: &str) -> bool {
    text.split_once("://").is_some_and(|(scheme, _)| SCHEMES.iter().any(|known| scheme.eq_ignore_ascii_case(known)))
}

/// A GVfs object, released when dropped.
struct Owned(&'static Gio, Object);

impl Owned {
    fn new(gio: &'static Gio, object: Object) -> Option<Self> {
        // Lazily: a null pointer must never reach `drop`.
        (!object.is_null()).then(|| Self(gio, object))
    }
}

impl Drop for Owned {
    fn drop(&mut self) {
        // SAFETY: the pointer came with a reference this value owns.
        unsafe { (self.0.g_object_unref)(self.1) }
    }
}

/// Async calls complete in the context that is the thread default when they start.
struct ThreadContext(&'static Gio, Object);

impl ThreadContext {
    fn new(gio: &'static Gio) -> Self {
        // SAFETY: a new context, made default for this thread until dropped.
        unsafe {
            let context = (gio.g_main_context_new)();
            (gio.g_main_context_push_thread_default)(context);
            Self(gio, context)
        }
    }

    fn run_until(&self, done: &dyn Fn() -> bool) {
        while !done() {
            // SAFETY: the context belongs to this thread.
            unsafe { (self.0.g_main_context_iteration)(self.1, 1) };
        }
    }
}

impl Drop for ThreadContext {
    fn drop(&mut self) {
        // SAFETY: pushed by `new` on this thread.
        unsafe {
            (self.0.g_main_context_pop_thread_default)(self.1);
            (self.0.g_main_context_unref)(self.1);
        }
    }
}

struct Failure {
    code: Option<i32>,
    message: String,
}

/// Takes the error, `code` set for the GIO domain alone.
unsafe fn failure(gio: &Gio, error: *mut GError) -> Failure {
    // SAFETY: GIO filled `error`, which the caller owns.
    unsafe {
        let code = ((*error).domain == (gio.g_io_error_quark)()).then_some((*error).code);
        let message = borrowed((*error).message);
        (gio.g_error_free)(error);
        Failure { code, message }
    }
}

fn remote_error(failure: Failure) -> RemoteError {
    match failure.code {
        Some(CANCELLED | FAILED_HANDLED) => RemoteError::Cancelled,
        _ => RemoteError::Other(failure.message),
    }
}

unsafe fn borrowed(text: *const c_char) -> String {
    if text.is_null() {
        return String::new();
    }
    // SAFETY: GIO strings end with a NUL.
    unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned()
}

unsafe fn taken(gio: &Gio, text: *mut c_char) -> Option<Vec<u8>> {
    if text.is_null() {
        return None;
    }
    // SAFETY: an allocated GIO string, freed once read.
    unsafe {
        let bytes = CStr::from_ptr(text).to_bytes().to_vec();
        (gio.g_free)(text.cast());
        Some(bytes)
    }
}

fn text(value: &str) -> CString {
    CString::new(value.replace('\0', "")).unwrap_or_default()
}

fn file_for_uri(gio: &'static Gio, uri: &str) -> Result<Owned, RemoteError> {
    // SAFETY: GIO parses any text; an invalid URI gives a file whose calls fail.
    Owned::new(gio, unsafe { (gio.g_file_new_for_uri)(text(uri).as_ptr()) })
        .ok_or_else(|| RemoteError::Other(format!("Invalid location {uri}")))
}

fn local_path(gio: &Gio, file: &Owned) -> Option<PathBuf> {
    // SAFETY: a live file; the path is copied then freed.
    unsafe { taken(gio, (gio.g_file_get_path)(file.1)) }.map(|bytes| PathBuf::from(std::ffi::OsString::from_vec(bytes)))
}

fn uri_of(gio: &Gio, file: Object) -> String {
    // SAFETY: a live file; the URI is copied then freed.
    unsafe { taken(gio, (gio.g_file_get_uri)(file)) }.map(|bytes| String::from_utf8_lossy(&bytes).into_owned()).unwrap_or_default()
}

/// Mounts the location when GVfs has not, asking `ask` whenever the server wants
/// credentials, again after a refused answer.
pub(super) fn connect(uri: &str, ask: &mut dyn FnMut(&RemotePrompt) -> RemoteAnswer) -> Result<RemoteLocation, RemoteError> {
    let gio = gio().ok_or(RemoteError::Unsupported)?;
    let file = file_for_uri(gio, uri)?;
    mount(gio, &file, ask)?;
    Ok(match local_path(gio, &file) {
        Some(path) => {
            appear(&path);
            RemoteLocation::Local(path)
        }
        None => RemoteLocation::Browse(uri_of(gio, file.1)),
    })
}

/// `gvfsd-fuse` learns of a mount by signal, after GIO: its folder can lag behind,
/// especially right after the same share was unmounted.
fn appear(path: &Path) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while std::fs::symlink_metadata(path).is_err() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

struct Asking<'a> {
    gio: &'static Gio,
    ask: &'a mut dyn FnMut(&RemotePrompt) -> RemoteAnswer,
    asked: bool,
}

#[derive(Default)]
struct Finished {
    done: std::cell::Cell<bool>,
    error: std::cell::Cell<Option<Failure>>,
}

fn mount(gio: &'static Gio, file: &Owned, ask: &mut dyn FnMut(&RemotePrompt) -> RemoteAnswer) -> Result<(), RemoteError> {
    let context = ThreadContext::new(gio);
    // SAFETY: a new operation, released at the end of this call.
    let operation = Owned::new(gio, unsafe { (gio.g_mount_operation_new)() }).ok_or(RemoteError::Unsupported)?;
    let mut asking = Asking { gio, ask, asked: false };
    let finished = Finished::default();
    // SAFETY: `asking` and `finished` outlive the loop that runs both callbacks; the
    // handler is disconnected before `asking` goes out of scope.
    let result = unsafe {
        let handler = (gio.g_signal_connect_data)(
            operation.1, c"ask-password".as_ptr(), ask_password as *const c_void,
            (&raw mut asking).cast(), std::ptr::null(), 0,
        );
        (gio.g_file_mount_enclosing_volume)(file.1, 0, operation.1, std::ptr::null_mut(), Some(mounted), (&raw const finished).cast_mut().cast());
        context.run_until(&|| finished.done.get());
        (gio.g_signal_handler_disconnect)(operation.1, handler);
        finished.error.take()
    };
    match result {
        None => Ok(()),
        Some(Failure { code: Some(ALREADY_MOUNTED), .. }) => Ok(()),
        Some(failure) => Err(remote_error(failure)),
    }
}

unsafe extern "C" fn mounted(source: Object, result: Object, data: *mut c_void) {
    // SAFETY: `data` is the `Finished` of `mount`, still on its stack.
    unsafe {
        let finished = &*data.cast::<Finished>();
        let gio = gio().expect("GIO loaded before mounting");
        let mut error = std::ptr::null_mut();
        if (gio.g_file_mount_enclosing_volume_finish)(source, result, &mut error) == 0 && !error.is_null() {
            finished.error.set(Some(failure(gio, error)));
        }
        finished.done.set(true);
    }
}

unsafe extern "C" fn ask_password(
    operation: Object, message: *const c_char, user: *const c_char, domain: *const c_char,
    flags: c_uint, data: *mut c_void,
) {
    // SAFETY: `data` is the `Asking` of `mount`; GIO strings are NUL-terminated.
    unsafe {
        let asking = &mut *data.cast::<Asking>();
        let prompt = RemotePrompt {
            message: borrowed(message),
            user: borrowed(user),
            domain: borrowed(domain),
            needs_password: flags & 1 != 0,
            needs_user: flags & 2 != 0,
            needs_domain: flags & 4 != 0,
            saving: flags & 8 != 0,
            anonymous: flags & 16 != 0,
            retry: asking.asked,
        };
        asking.asked = true;
        let gio = asking.gio;
        // GMountOperationResult: handled, aborted.
        let reply = match (asking.ask)(&prompt) {
            RemoteAnswer::Login { user, domain, password, save } => {
                (gio.g_mount_operation_set_anonymous)(operation, 0);
                (gio.g_mount_operation_set_username)(operation, text(&user).as_ptr());
                (gio.g_mount_operation_set_domain)(operation, text(&domain).as_ptr());
                (gio.g_mount_operation_set_password)(operation, text(&password).as_ptr());
                // GPasswordSave, in declaration order.
                let save = match save {
                    PasswordSave::Never => 0,
                    PasswordSave::Session => 1,
                    PasswordSave::Permanently => 2,
                };
                (gio.g_mount_operation_set_password_save)(operation, save);
                0
            }
            RemoteAnswer::Anonymous => {
                (gio.g_mount_operation_set_anonymous)(operation, 1);
                0
            }
            RemoteAnswer::Cancel => 1,
        };
        (gio.g_mount_operation_reply)(operation, reply);
    }
}

/// A server, share or folder of the network list, by the URI that opens it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RemoteChild {
    pub name: String,
    pub uri: String,
    pub directory: bool,
}

/// Lists a mounted location, such as `network:///` or `smb://server/`.
pub(super) fn children(uri: &str) -> Result<Vec<RemoteChild>, RemoteError> {
    let gio = gio().ok_or(RemoteError::Unsupported)?;
    let parent = file_for_uri(gio, uri)?;
    // A refresh or a reopened tab mounts again with saved credentials, never a prompt.
    mount(gio, &parent, &mut |_| RemoteAnswer::Cancel)?;
    // The neighborhood points at servers. A server's shares are named from its URI: their
    // target drops a non-default port and GIO's child of a server is `._share`.
    let targets = uri.get(..10).is_some_and(|scheme| scheme.eq_ignore_ascii_case("network://"));
    let mut error = std::ptr::null_mut();
    // SAFETY: a live file; the enumerator and each info are released after use.
    unsafe {
        let enumerator = (gio.g_file_enumerate_children)(
            parent.1, c"standard::name,standard::display-name,standard::type,standard::target-uri".as_ptr(),
            0, std::ptr::null_mut(), &mut error,
        );
        let enumerator = Owned::new(gio, enumerator).ok_or_else(|| remote_error(failure(gio, error)))?;
        let mut children = Vec::new();
        loop {
            let info = (gio.g_file_enumerator_next_file)(enumerator.1, std::ptr::null_mut(), &mut error);
            let Some(info) = Owned::new(gio, info) else {
                if !error.is_null() {
                    return Err(remote_error(failure(gio, error)));
                }
                return Ok(children);
            };
            let kind = (gio.g_file_info_get_file_type)(info.1);
            let target = borrowed((gio.g_file_info_get_attribute_string)(info.1, c"standard::target-uri".as_ptr()));
            let uri = if targets && !target.is_empty() {
                target
            } else if let Some((_, share)) = target.trim_end_matches('/').rsplit_once('/').filter(|_| kind == MOUNTABLE) {
                // A share of this server, by the server URI that keeps its port.
                format!("{}/{share}", uri.trim_end_matches('/'))
            } else {
                let child = Owned::new(gio, (gio.g_file_get_child)(parent.1, (gio.g_file_info_get_name)(info.1)));
                child.map(|child| uri_of(gio, child.1)).unwrap_or_default()
            };
            if uri.is_empty() {
                continue;
            }
            children.push(RemoteChild {
                name: borrowed((gio.g_file_info_get_display_name)(info.1)),
                uri,
                directory: matches!(kind, DIRECTORY | SHORTCUT | MOUNTABLE),
            });
        }
    }
}

/// A network share mounted by GVfs, whoever asked for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RemoteMount {
    pub path: PathBuf,
    pub name: String,
    pub uri: String,
}

/// GVfs mounts appear as folders of one FUSE mount, named after their mount spec.
pub(super) fn fuse_root() -> Option<PathBuf> {
    dirs::runtime_dir().map(|runtime| runtime.join("gvfs"))
}

pub(super) fn network_mount_name(name: &std::ffi::OsStr) -> bool {
    name.as_bytes().split(|&byte| byte == b':').next()
        .is_some_and(|kind| NETWORK_MOUNTS.iter().any(|known| known.as_bytes() == kind))
}

pub(super) fn mounts() -> Vec<RemoteMount> {
    let (Some(gio), Some(root)) = (gio(), fuse_root()) else { return Vec::new() };
    let Ok(entries) = std::fs::read_dir(&root) else { return Vec::new() };
    entries.flatten()
        .filter(|entry| network_mount_name(&entry.file_name()))
        .filter_map(|entry| {
            let path = entry.path();
            let mount = enclosing_mount(gio, &path).ok()?;
            // SAFETY: a live mount; its name and root are copied then released.
            unsafe {
                let name = taken(gio, (gio.g_mount_get_name)(mount.1)).map(|bytes| String::from_utf8_lossy(&bytes).into_owned())?;
                let root = Owned::new(gio, (gio.g_mount_get_root)(mount.1))?;
                Some(RemoteMount { path, name, uri: uri_of(gio, root.1) })
            }
        })
        .collect()
}

fn enclosing_mount(gio: &'static Gio, path: &Path) -> Result<Owned, Failure> {
    let path = CString::new(path.as_os_str().as_bytes()).map_err(|_| Failure { code: None, message: "Invalid path".into() })?;
    let mut error = std::ptr::null_mut();
    // SAFETY: GIO maps a FUSE path back to its GVfs mount; both objects are released.
    unsafe {
        let file = Owned::new(gio, (gio.g_file_new_for_path)(path.as_ptr()))
            .ok_or_else(|| Failure { code: None, message: "Invalid path".into() })?;
        let mount = (gio.g_file_find_enclosing_mount)(file.1, std::ptr::null_mut(), &mut error);
        Owned::new(gio, mount).ok_or_else(|| failure(gio, error))
    }
}

/// Unmounts the GVfs share holding `path`.
pub(super) fn unmount(path: &Path) -> Result<(), VolumeError> {
    let gio = gio().ok_or_else(|| VolumeError::Other("GIO unavailable".into()))?;
    // GVfs no longer knows the share: unmounted meanwhile, by Nautilus for instance, while
    // its FUSE folder can linger a moment.
    let Ok(mount) = enclosing_mount(gio, path) else { return Ok(()) };
    let context = ThreadContext::new(gio);
    let finished = Finished::default();
    // SAFETY: `finished` outlives the loop that runs the callback.
    unsafe {
        (gio.g_mount_unmount_with_operation)(mount.1, 0, std::ptr::null_mut(), std::ptr::null_mut(), Some(unmounted), (&raw const finished).cast_mut().cast());
    }
    context.run_until(&|| finished.done.get());
    match finished.error.take() {
        None => Ok(()),
        Some(Failure { code: Some(BUSY), .. }) => Err(VolumeError::Busy(Vec::new())),
        Some(Failure { code: Some(CANCELLED | FAILED_HANDLED), .. }) => Err(VolumeError::Cancelled),
        Some(failure) => Err(VolumeError::Other(failure.message)),
    }
}

unsafe extern "C" fn unmounted(source: Object, result: Object, data: *mut c_void) {
    // SAFETY: `data` is the `Finished` of `unmount`, still on its stack.
    unsafe {
        let finished = &*data.cast::<Finished>();
        let gio = gio().expect("GIO loaded before unmounting");
        let mut error = std::ptr::null_mut();
        if (gio.g_mount_unmount_with_operation_finish)(source, result, &mut error) == 0 && !error.is_null() {
            finished.error.set(Some(failure(gio, error)));
        }
        finished.done.set(true);
    }
}

#[cfg(test)]
#[path = "linux_gio/tests.rs"]
mod tests;
