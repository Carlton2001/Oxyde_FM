//! Codec workers: zero-capability LPAC, explicit inherited handles and a job
//! attached while the initial thread is suspended. No permissive fallback.
use std::ffi::OsStr;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle};
use std::os::windows::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Security::Authorization::*;
use windows::Win32::Security::Isolation::*;
use windows::Win32::Security::*;
use windows::Win32::Storage::FileSystem::{
    FileDispositionInfo, FileIdInfo, GetFileInformationByHandleEx, SetFileInformationByHandle,
    FILE_DISPOSITION_INFO, FILE_ID_INFO,
};
use windows::Win32::System::JobObjects::AssignProcessToJobObject;
use windows::Win32::System::Pipes::CreatePipe;
use windows::Win32::System::Threading::*;

fn native(e: windows::core::Error) -> io::Error {
    io::Error::other(e.to_string())
}
fn wide(value: impl AsRef<OsStr>) -> Vec<u16> {
    value.as_ref().encode_wide().chain(Some(0)).collect()
}
fn owned(raw: HANDLE) -> OwnedHandle {
    unsafe { OwnedHandle::from_raw_handle(raw.0) }
}

struct Profile {
    sid: PSID,
    name: Vec<u16>,
    registered: bool,
}
impl Profile {
    fn create() -> io::Result<Self> {
        let name = wide(format!("Oxyde.Archive.{}", uuid::Uuid::new_v4()));
        // A fresh moniker determines a unique package SID. The parent creates
        // the bootstrap store and removes it before the initial thread resumes.
        let sid = unsafe {
            CreateAppContainerProfile(
                PCWSTR(name.as_ptr()),
                PCWSTR(name.as_ptr()),
                PCWSTR(name.as_ptr()),
                None,
            )
        }
        .map_err(native)?;
        Ok(Self {
            sid,
            name,
            registered: true,
        })
    }
    fn remove_before_execution(&mut self) -> io::Result<()> {
        unsafe { DeleteAppContainerProfile(PCWSTR(self.name.as_ptr())) }.map_err(native)?;
        self.registered = false;
        Ok(())
    }
}
impl Drop for Profile {
    fn drop(&mut self) {
        if self.registered {
            if let Err(error) = self.remove_before_execution() {
                log::warn!(
                    "Archive LPAC bootstrap profile residue: {}: {error}",
                    String::from_utf16_lossy(&self.name)
                );
            }
        }
        unsafe {
            FreeSid(self.sid);
        }
    }
}

/// Handles prevent replacing the executable and staging directory until exit.
/// Staging is always private; no checkout or global ACL is changed.
struct Runtime {
    binary: Option<File>,
    directory: Option<File>,
    _parents: Vec<File>,
    root: PathBuf,
}
impl Runtime {
    fn create(
        source: &Path,
        sid: PSID,
        checkpoint: &dyn Fn() -> io::Result<()>,
    ) -> io::Result<Self> {
        checkpoint()?;
        let mut parents = retain_parents(source)?;
        let mut source_file = retain(source, false)?;
        const MAX_RUNTIME_BYTES: u64 = 128 * 1024 * 1024;
        if source_file.metadata()?.len() > MAX_RUNTIME_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::FileTooLarge,
                "Worker runtime exceeds its limit",
            ));
        }
        let temporary = std::env::temp_dir();
        parents.extend(retain_parents(&temporary)?);
        let temp_handle = OpenOptions::new()
            .read(true)
            .share_mode(3)
            .custom_flags(0x00200000 | 0x02000000)
            .open(&temporary)?;
        if temp_handle.metadata()?.file_attributes() & 0x400 != 0 {
            return Err(io::Error::other(
                "Archive temporary directory is a reparse point",
            ));
        }
        parents.push(temp_handle);
        let root = tempfile::Builder::new()
            .prefix("oxyde-archive-lpac-")
            .tempdir_in(&temporary)?
            .keep();
        // From creation onward every failure uses the same exact-object cleanup.
        let mut runtime = Self {
            binary: None,
            directory: None,
            _parents: parents,
            root,
        };
        runtime.directory = Some(retain_for_delete(&runtime.root, true)?);
        let mut sid_string = PWSTR::null();
        unsafe { ConvertSidToStringSidW(sid, &mut sid_string) }.map_err(native)?;
        let sid_text = unsafe { sid_string.to_string() }.map_err(io::Error::other);
        unsafe {
            LocalFree(Some(HLOCAL(sid_string.0.cast())));
        }
        let sddl = wide(format!(
            "D:P(A;OICI;FA;;;{})(A;OICI;FA;;;SY)(A;OICI;GRGX;;;{})",
            parent_user_sid()?,
            sid_text?
        ));
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PCWSTR(sddl.as_ptr()),
                1,
                &mut descriptor,
                None,
            )
        }
        .map_err(native)?;
        let path = wide(&runtime.root);
        let result = unsafe {
            SetFileSecurityW(
                PCWSTR(path.as_ptr()),
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                descriptor,
            )
        }
        .ok()
        .map_err(native);
        unsafe {
            LocalFree(Some(HLOCAL(descriptor.0)));
        }
        result?;
        let target = runtime.root.join("worker.exe");
        runtime.binary = Some(
            OpenOptions::new()
                .write(true)
                .access_mode(0x80000000 | 0x40000000 | 0x10000)
                .create_new(true)
                .share_mode(1)
                .open(&target)?,
        );
        let target_file = runtime
            .binary
            .as_mut()
            .ok_or_else(|| io::Error::other("Missing staged binary"))?;
        use std::io::{Read, Seek, SeekFrom};
        let mut copy_bytes = [0; 64 * 1024];
        let mut copied = 0u64;
        loop {
            checkpoint()?;
            let count = source_file.read(&mut copy_bytes)?;
            if count == 0 {
                break;
            }
            copied += count as u64;
            if copied > MAX_RUNTIME_BYTES {
                return Err(io::Error::new(
                    io::ErrorKind::FileTooLarge,
                    "Worker runtime exceeds its limit",
                ));
            }
            target_file.write_all(&copy_bytes[..count])?;
        }
        target_file.sync_all()?;
        // Overlap handles while downgrading from the creator's write access.
        // An image cannot be loaded while a writable file object is retained.
        let transition = OpenOptions::new().read(true).share_mode(7).open(&target)?;
        let created_id = file_id(target_file)?;
        if file_id(&transition)? != created_id {
            return Err(io::Error::other("Archive runtime identity changed"));
        }
        drop(runtime.binary.take());
        #[cfg(test)]
        HANDLE_TRANSITION_HOOK.with(|hook| {
            if let Some(hook) = hook.borrow_mut().take() {
                hook(
                    &target,
                    runtime
                        .directory
                        .as_ref()
                        .expect("retained runtime directory"),
                );
            }
        });
        let binary = retain_for_delete(&target, false)?;
        if file_id(&binary)? != created_id {
            // The reopened file is foreign: never enroll it in owned cleanup.
            return Err(io::Error::other("Archive runtime identity changed"));
        }
        drop(transition);
        runtime.binary = Some(binary);
        let binary = runtime
            .binary
            .as_ref()
            .ok_or_else(|| io::Error::other("Missing staged binary"))?;
        // Compare bytes through retained handles, never a second pathname read.
        source_file.seek(SeekFrom::Start(0))?;
        let hash = |mut file: &File| -> io::Result<blake3::Hash> {
            let mut hasher = blake3::Hasher::new();
            let mut bytes = [0; 64 * 1024];
            loop {
                checkpoint()?;
                let count = file.read(&mut bytes)?;
                if count == 0 {
                    break;
                }
                hasher.update(&bytes[..count]);
            }
            Ok(hasher.finalize())
        };
        if hash(&source_file)? != hash(binary)? {
            return Err(io::Error::other("Archive runtime provenance mismatch"));
        }
        Ok(runtime)
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        // Delete only retained identities, never recurse or rediscover a path.
        if let Some(binary) = self.binary.take() {
            if let Err(error) = mark_delete(&binary) {
                log::warn!(
                    "Archive runtime file residue at {}: {error}",
                    self.root.display()
                );
            }
            drop(binary);
        }
        if let Some(directory) = self.directory.take() {
            if let Err(error) = mark_delete(&directory) {
                log::warn!(
                    "Archive runtime directory residue at {}: {error}",
                    self.root.display()
                );
            }
            drop(directory);
        }
    }
}
fn mark_delete(file: &File) -> io::Result<()> {
    let disposition = FILE_DISPOSITION_INFO { DeleteFile: true };
    unsafe {
        SetFileInformationByHandle(
            HANDLE(file.as_raw_handle()),
            FileDispositionInfo,
            (&disposition as *const FILE_DISPOSITION_INFO).cast(),
            std::mem::size_of_val(&disposition) as u32,
        )
    }
    .map_err(native)
}
fn file_id(file: &File) -> io::Result<(u64, [u8; 16])> {
    let mut info = FILE_ID_INFO::default();
    unsafe {
        GetFileInformationByHandleEx(
            HANDLE(file.as_raw_handle()),
            FileIdInfo,
            (&mut info as *mut FILE_ID_INFO).cast(),
            std::mem::size_of_val(&info) as u32,
        )
    }
    .map_err(native)?;
    Ok((info.VolumeSerialNumber, info.FileId.Identifier))
}
fn retain_for_delete(path: &Path, directory: bool) -> io::Result<File> {
    let file = OpenOptions::new()
        .access_mode(0x80000000 | 0x10000)
        .share_mode(1)
        .custom_flags(0x00200000 | if directory { 0x02000000 } else { 0 })
        .open(path)?;
    let metadata = file.metadata()?;
    if metadata.file_attributes() & 0x400 != 0 || metadata.is_dir() != directory {
        return Err(io::Error::other(
            "Archive runtime reparse point or unexpected type",
        ));
    }
    Ok(file)
}
fn retain_parents(path: &Path) -> io::Result<Vec<File>> {
    if !path.is_absolute() {
        return Err(io::Error::other("Archive runtime path must be absolute"));
    }
    let mut parents = Vec::new();
    for parent in path
        .ancestors()
        .skip(1)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
    {
        let file = OpenOptions::new()
            .read(true)
            .share_mode(3)
            .custom_flags(0x00200000 | 0x02000000)
            .open(parent)?;
        if file.metadata()?.file_attributes() & 0x400 != 0 {
            return Err(io::Error::other(
                "Archive runtime ancestor is a reparse point",
            ));
        }
        parents.push(file);
    }
    Ok(parents)
}
fn retain(path: &Path, directory: bool) -> io::Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .share_mode(1)
        .custom_flags(0x00200000 | if directory { 0x02000000 } else { 0 })
        .open(path)?;
    let metadata = file.metadata()?;
    if metadata.file_attributes() & 0x400 != 0 || metadata.is_dir() != directory {
        return Err(io::Error::other(
            "Archive runtime reparse point or unexpected type",
        ));
    }
    Ok(file)
}

fn parent_user_sid() -> io::Result<String> {
    let mut token = HANDLE::default();
    unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) }.map_err(native)?;
    let token = owned(token);
    let mut bytes = 0;
    let _ = unsafe {
        GetTokenInformation(
            HANDLE(token.as_raw_handle()),
            TokenUser,
            None,
            0,
            &mut bytes,
        )
    };
    if bytes == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut buffer = vec![0usize; (bytes as usize).div_ceil(std::mem::size_of::<usize>())];
    unsafe {
        GetTokenInformation(
            HANDLE(token.as_raw_handle()),
            TokenUser,
            Some(buffer.as_mut_ptr().cast()),
            bytes,
            &mut bytes,
        )
    }
    .map_err(native)?;
    let user = unsafe { &*buffer.as_ptr().cast::<TOKEN_USER>() };
    let mut text = PWSTR::null();
    unsafe { ConvertSidToStringSidW(user.User.Sid, &mut text) }.map_err(native)?;
    let result = unsafe { text.to_string() }.map_err(io::Error::other);
    unsafe {
        LocalFree(Some(HLOCAL(text.0.cast())));
    }
    result
}

struct Attributes<'a> {
    _storage: Vec<usize>,
    list: LPPROC_THREAD_ATTRIBUTE_LIST,
    _values: std::marker::PhantomData<&'a ()>,
}
impl<'a> Attributes<'a> {
    fn new() -> io::Result<Self> {
        let mut size = 0;
        let _ = unsafe { InitializeProcThreadAttributeList(None, 3, None, &mut size) };
        if size == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut storage = vec![0usize; size.div_ceil(std::mem::size_of::<usize>())];
        let list = LPPROC_THREAD_ATTRIBUTE_LIST(storage.as_mut_ptr().cast());
        unsafe { InitializeProcThreadAttributeList(Some(list), 3, None, &mut size) }
            .map_err(native)?;
        Ok(Self {
            _storage: storage,
            list,
            _values: std::marker::PhantomData,
        })
    }
    fn set<T>(&mut self, attribute: u32, values: &'a [T]) -> io::Result<()> {
        unsafe {
            UpdateProcThreadAttribute(
                self.list,
                0,
                attribute as usize,
                Some(values.as_ptr().cast()),
                std::mem::size_of_val(values),
                None,
                None,
            )
        }
        .map_err(native)
    }
}
impl Drop for Attributes<'_> {
    fn drop(&mut self) {
        unsafe {
            DeleteProcThreadAttributeList(self.list);
        }
    }
}

pub(super) struct Process {
    handle: OwnedHandle,
}
impl AsRawHandle for Process {
    fn as_raw_handle(&self) -> RawHandle {
        self.handle.as_raw_handle()
    }
}
impl Process {
    pub(super) fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        match unsafe { WaitForSingleObject(HANDLE(self.as_raw_handle()), 0) } {
            WAIT_TIMEOUT => {
                super::windows_resource_pressure::check_worker(HANDLE(self.as_raw_handle()))?;
                Ok(None)
            }
            WAIT_OBJECT_0 => {
                let mut code = 0;
                unsafe { GetExitCodeProcess(HANDLE(self.as_raw_handle()), &mut code) }
                    .map_err(native)?;
                Ok(Some(ExitStatus::from_raw(code)))
            }
            _ => Err(io::Error::last_os_error()),
        }
    }
}
pub(super) struct Worker {
    pub(super) child: Process,
    pub(super) _job: OwnedHandle,
    runtime: Option<Runtime>,
    profile: Option<Profile>,
    budget: Option<super::windows_worker_budget::Permit>,
}
enum LaunchKind {
    Lpac,
    #[cfg(test)]
    AppContainerControl,
    #[cfg(test)]
    LpacCapabilityProbe(&'static [&'static str]),
}
impl Worker {
    #[cfg(test)]
    pub(super) fn start_duplex(command: Command, memory: usize) -> io::Result<(Self, File, File)> {
        Self::start_duplex_checked(command, memory, &|| Ok(()))
    }
    pub(super) fn start_duplex_checked(
        command: Command,
        memory: usize,
        checkpoint: &dyn Fn() -> io::Result<()>,
    ) -> io::Result<(Self, File, File)> {
        Self::start_duplex_with_kind(command, memory, checkpoint, LaunchKind::Lpac, None)
    }
    pub(super) fn start_duplex_source(
        command: Command,
        memory: usize,
        checkpoint: &dyn Fn() -> io::Result<()>,
        source: &File,
    ) -> io::Result<(Self, File, File)> {
        Self::start_duplex_with_kind(command, memory, checkpoint, LaunchKind::Lpac, Some(source))
    }
    #[cfg(test)]
    fn start_duplex_for_test(
        command: Command,
        memory: usize,
        lpac: bool,
    ) -> io::Result<(Self, File, File)> {
        Self::start_duplex_with_kind(
            command,
            memory,
            &|| Ok(()),
            if lpac {
                LaunchKind::Lpac
            } else {
                LaunchKind::AppContainerControl
            },
            None,
        )
    }
    fn start_duplex_with_kind(
        command: Command,
        memory: usize,
        checkpoint: &dyn Fn() -> io::Result<()>,
        kind: LaunchKind,
        source: Option<&File>,
    ) -> io::Result<(Self, File, File)> {
        checkpoint()?;
        let budget = super::windows_worker_budget::acquire(memory, checkpoint)?;
        let profile = Profile::create()?;
        let runtime = Runtime::create(Path::new(command.get_program()), profile.sid, checkpoint)?;
        checkpoint()?;
        let job = super::windows_preview_worker::job(memory)?;
        let (parent_read, child_write) = pipe()?;
        let (child_read, parent_write) = pipe()?;
        // stderr is a distinct pipe whose parent reader is immediately closed.
        let (stderr_read, stderr_write) = pipe()?;
        drop(stderr_read);
        #[cfg(test)]
        let capability_probe = if let LaunchKind::LpacCapabilityProbe(names) = &kind {
            Some(capability_probe::Capabilities::new(names)?)
        } else {
            None
        };
        let caps = SECURITY_CAPABILITIES {
            AppContainerSid: profile.sid,
            ..Default::default()
        };
        #[cfg(test)]
        let caps = {
            let mut caps = caps;
            if let Some(probe) = &capability_probe {
                probe.apply(&mut caps);
            }
            caps
        };
        let policy = match kind {
            LaunchKind::Lpac => 1u32,
            #[cfg(test)]
            LaunchKind::AppContainerControl => 0u32,
            #[cfg(test)]
            LaunchKind::LpacCapabilityProbe(_) => 1u32,
        };
        let executable = runtime.root.join("worker.exe");
        // Source is already opened read-only and retained by the parent. This
        // explicit extra handle grants no pathname access or writable object.
        let source = source
            .map(|file| {
                unsafe { std::os::windows::io::BorrowedHandle::borrow_raw(file.as_raw_handle()) }
                    .try_clone_to_owned()
            })
            .transpose()?;
        let app = wide(&executable);
        let mut line = quote(executable.as_os_str());
        for argument in command.get_args() {
            line.push(32);
            line.extend(quote(argument));
        }
        if let Some(source) = &source {
            line.extend(
                wide(format!(
                    " --source-handle {}",
                    source.as_raw_handle() as usize
                ))
                .into_iter()
                .take_while(|unit| *unit != 0),
            );
        }
        line.push(0);
        let cwd = wide(&runtime.root);
        // No inherited environment: Windows loader resources only.
        let mut system = [0u16; 32768];
        let length = unsafe {
            windows::Win32::System::SystemInformation::GetWindowsDirectoryW(Some(&mut system))
        } as usize;
        if length == 0 || length >= system.len() {
            return Err(io::Error::last_os_error());
        }
        let root = String::from_utf16(&system[..length]).map_err(io::Error::other)?;
        // AppContainer creation requires LOCALAPPDATA to route its environment.
        // This names a directory; it grants no access to the parent's files.
        let local = unsafe {
            windows::Win32::UI::Shell::SHGetKnownFolderPath(
                &windows::Win32::UI::Shell::FOLDERID_LocalAppData,
                windows::Win32::UI::Shell::KF_FLAG_DEFAULT,
                None,
            )
        }
        .map_err(native)?;
        let local_text = unsafe { local.to_string() }.map_err(io::Error::other);
        unsafe {
            windows::Win32::System::Com::CoTaskMemFree(Some(local.0.cast()));
        }
        let environment = wide(format!(
            "LOCALAPPDATA={}\0SystemRoot={root}\0WINDIR={root}\0",
            local_text?
        ));
        // No callbacks or environment discovery while handles are inheritable.
        checkpoint()?;
        let mut child_handles = vec![child_read, child_write, stderr_write];
        if let Some(source) = source {
            child_handles.push(source);
        }
        let inherited = super::windows_process_launch::InheritedHandles::new(child_handles)?;
        let handles = inherited.raw();
        let mut attributes = Attributes::new()?;
        attributes.set(
            PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES,
            std::slice::from_ref(&caps),
        )?;
        attributes.set(
            PROC_THREAD_ATTRIBUTE_ALL_APPLICATION_PACKAGES_POLICY,
            std::slice::from_ref(&policy),
        )?;
        attributes.set(PROC_THREAD_ATTRIBUTE_HANDLE_LIST, &handles)?;
        let mut start = STARTUPINFOEXW::default();
        start.StartupInfo.cb = std::mem::size_of_val(&start) as u32;
        start.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
        start.StartupInfo.hStdInput = handles[0];
        start.StartupInfo.hStdOutput = handles[1];
        start.StartupInfo.hStdError = handles[2];
        start.lpAttributeList = attributes.list;
        let mut info = PROCESS_INFORMATION::default();
        unsafe {
            CreateProcessW(
                PCWSTR(app.as_ptr()),
                Some(PWSTR(line.as_mut_ptr())),
                None,
                None,
                true,
                CREATE_SUSPENDED
                    | CREATE_NO_WINDOW
                    | CREATE_UNICODE_ENVIRONMENT
                    | EXTENDED_STARTUPINFO_PRESENT,
                Some(environment.as_ptr().cast()),
                PCWSTR(cwd.as_ptr()),
                &start.StartupInfo,
                &mut info,
            )
        }
        .map_err(|error| io::Error::other(format!("LPAC CreateProcess: {error}")))?;
        drop(attributes); // Attribute values and SID remain alive until this point.
        drop(inherited); // Close child endpoints before releasing the shared lock.
        let thread = owned(info.hThread);
        let mut worker = Self {
            child: Process {
                handle: owned(info.hProcess),
            },
            _job: job,
            runtime: Some(runtime),
            profile: Some(profile),
            budget: Some(budget),
        };
        super::windows_worker_budget::attach(HANDLE(worker.child.as_raw_handle()))?;
        unsafe {
            AssignProcessToJobObject(
                HANDLE(worker._job.as_raw_handle()),
                HANDLE(worker.child.as_raw_handle()),
            )
        }
        .map_err(native)?;
        checkpoint()?;
        // The initial thread is still suspended. Remove the bootstrap profile
        // before any untrusted code can populate its filesystem or registry.
        worker
            .profile
            .as_mut()
            .ok_or_else(|| io::Error::other("Missing LPAC identity"))?
            .remove_before_execution()?;
        if unsafe { ResumeThread(HANDLE(thread.as_raw_handle())) } == u32::MAX {
            return Err(io::Error::last_os_error());
        }
        let mut input = File::from(parent_write);
        input.write_all(&[0xA7])?;
        // Keep mut binding local to ensure failure always executes Worker::drop.
        let _ = &mut worker;
        Ok((worker, File::from(parent_read), input))
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        let handle = HANDLE(self.child.as_raw_handle());
        let _ = unsafe { TerminateProcess(handle, 1) };
        if unsafe { WaitForSingleObject(handle, 1000) } != WAIT_OBJECT_0 {
            let runtime = self.runtime.take();
            if let Some(runtime) = &runtime {
                log::warn!(
                    "Archive sandbox termination delayed: PID {}, retaining private runtime at {}",
                    unsafe { GetProcessId(handle) },
                    runtime.root.display()
                );
            }
            if let Some(budget) = self.budget.take() {
                super::windows_worker_budget::retain_until_exit(handle, budget, move || {
                    drop(runtime)
                });
            } else {
                std::mem::forget(runtime);
            }
        } else if let Some(runtime) = self.runtime.take() {
            drop(runtime);
        }
    }
}
fn pipe() -> io::Result<(OwnedHandle, OwnedHandle)> {
    let mut read = HANDLE::default();
    let mut write = HANDLE::default();
    unsafe { CreatePipe(&mut read, &mut write, None, 2 * 1024 * 1024) }.map_err(native)?;
    Ok((owned(read), owned(write)))
}
fn quote(argument: &OsStr) -> Vec<u16> {
    let mut result = vec![34];
    let mut slashes = 0;
    for character in argument.encode_wide() {
        if character == 92 {
            slashes += 1;
            continue;
        }
        result.extend(std::iter::repeat_n(
            92,
            if character == 34 {
                slashes * 2 + 1
            } else {
                slashes
            },
        ));
        slashes = 0;
        result.push(character);
    }
    result.extend(std::iter::repeat_n(92, slashes * 2));
    result.push(34);
    result
}

#[cfg(test)]
#[path = "windows_worker_sandbox/tests/capability_probe.rs"]
mod capability_probe;
#[cfg(test)]
#[path = "windows_worker_sandbox/tests/security_tests.rs"]
mod security_tests;

#[cfg(test)]
thread_local! {
    static HANDLE_TRANSITION_HOOK: std::cell::RefCell<Option<HandleTransitionHook>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
type HandleTransitionHook = Box<dyn FnOnce(&Path, &File)>;

#[cfg(test)]
#[path = "windows_worker_sandbox/tests/identity_tests.rs"]
mod identity_tests;
