//! Own clipboard allocations until Windows accepts each format.
use crate::models::CommandError;
use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, RegisterClipboardFormatA, SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::System::Ole::{CF_HDROP, CF_UNICODETEXT};
use windows::Win32::UI::Shell::DROPFILES;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, HWND_MESSAGE, WINDOW_EX_STYLE, WINDOW_STYLE,
};

type Result<T> = std::result::Result<T, CommandError>;

fn error(operation: &str, detail: impl std::fmt::Display) -> CommandError {
    CommandError::SystemError(format!("{operation}: {detail}"))
}

// A private boundary for fault injection: tests never open the user's clipboard.
trait ClipboardApi {
    fn allocate(&self, size: usize) -> Result<HGLOBAL>;
    fn lock(&self, handle: HGLOBAL) -> Result<*mut u8>;
    fn unlock(&self, handle: HGLOBAL);
    fn free(&self, handle: HGLOBAL);
    fn create_owner(&self) -> Result<HWND>;
    fn destroy_owner(&self, owner: HWND);
    fn open(&self, owner: HWND) -> Result<()>;
    fn empty(&self) -> Result<()>;
    fn publish(&self, format: u32, handle: HGLOBAL) -> Result<()>;
    fn close(&self) -> Result<()>;
    fn effect_format(&self) -> Result<u32>;
}

struct NativeApi;
impl ClipboardApi for NativeApi {
    fn allocate(&self, size: usize) -> Result<HGLOBAL> {
        unsafe { GlobalAlloc(GMEM_MOVEABLE, size) }
            .map_err(|e| error("Cannot allocate clipboard data", e))
    }
    fn lock(&self, handle: HGLOBAL) -> Result<*mut u8> {
        let pointer = unsafe { GlobalLock(handle) }.cast::<u8>();
        if pointer.is_null() {
            return Err(error(
                "Cannot lock clipboard data",
                std::io::Error::last_os_error(),
            ));
        }
        Ok(pointer)
    }
    fn unlock(&self, handle: HGLOBAL) {
        unsafe {
            let _ = GlobalUnlock(handle);
        }
    }
    fn free(&self, handle: HGLOBAL) {
        unsafe {
            let _ = GlobalFree(Some(handle));
        }
    }
    fn create_owner(&self) -> Result<HWND> {
        // OpenClipboard(NULL) + EmptyClipboard leaves no owner for SetClipboardData.
        // This message-only window is created and destroyed on the calling thread.
        unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                windows::core::w!("STATIC"),
                windows::core::w!("Oxyde clipboard"),
                WINDOW_STYLE::default(),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                None,
                None,
            )
        }
        .map_err(|e| error("Cannot create clipboard owner", e))
    }
    fn destroy_owner(&self, owner: HWND) {
        unsafe {
            let _ = DestroyWindow(owner);
        }
    }
    fn open(&self, owner: HWND) -> Result<()> {
        unsafe { OpenClipboard(Some(owner)) }.map_err(|e| error("Cannot open clipboard", e))
    }
    fn empty(&self) -> Result<()> {
        unsafe { EmptyClipboard() }.map_err(|e| error("Cannot empty clipboard", e))
    }
    fn publish(&self, format: u32, handle: HGLOBAL) -> Result<()> {
        unsafe { SetClipboardData(format, Some(HANDLE(handle.0))) }
            .map(|_| ())
            .map_err(|e| error("Cannot publish clipboard data", e))
    }
    fn close(&self) -> Result<()> {
        unsafe { CloseClipboard() }.map_err(|e| error("Cannot close clipboard", e))
    }
    fn effect_format(&self) -> Result<u32> {
        let format = unsafe {
            RegisterClipboardFormatA(windows::core::PCSTR(
                c"Preferred DropEffect".as_ptr().cast(),
            ))
        };
        if format == 0 {
            return Err(error(
                "Cannot register clipboard drop effect",
                std::io::Error::last_os_error(),
            ));
        }
        Ok(format)
    }
}

struct Buffer<'a> {
    api: &'a dyn ClipboardApi,
    handle: Option<HGLOBAL>,
}
impl<'a> Buffer<'a> {
    fn new(api: &'a dyn ClipboardApi, bytes: &[u8]) -> Result<Self> {
        let buffer = Self {
            api,
            handle: Some(api.allocate(bytes.len())?),
        };
        let handle = buffer.handle.unwrap();
        let pointer = api.lock(handle)?;
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), pointer, bytes.len());
        }
        api.unlock(handle);
        Ok(buffer)
    }
    fn publish(&mut self, format: u32) -> Result<()> {
        self.api.publish(format, self.handle.unwrap())?;
        // Only a successful publication transfers the allocation to Windows.
        self.handle = None;
        Ok(())
    }
}
impl Drop for Buffer<'_> {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            self.api.free(handle);
        }
    }
}

struct Owner<'a>(&'a dyn ClipboardApi, HWND);
impl Drop for Owner<'_> {
    fn drop(&mut self) {
        self.0.destroy_owner(self.1);
    }
}
struct Session<'a> {
    api: &'a dyn ClipboardApi,
    open: bool,
}
impl<'a> Session<'a> {
    fn new(api: &'a dyn ClipboardApi, owner: HWND) -> Result<Self> {
        api.open(owner)?;
        Ok(Self { api, open: true })
    }
    fn close(mut self) -> Result<()> {
        self.api.close()?;
        self.open = false;
        Ok(())
    }
}
impl Drop for Session<'_> {
    fn drop(&mut self) {
        if self.open {
            let _ = self.api.close();
        }
    }
}

pub(super) fn file_bytes(paths: &[String]) -> Vec<u8> {
    let header = DROPFILES {
        pFiles: std::mem::size_of::<DROPFILES>() as u32,
        fWide: true.into(),
        ..Default::default()
    };
    let mut bytes = vec![0; std::mem::size_of::<DROPFILES>()];
    unsafe {
        bytes
            .as_mut_ptr()
            .cast::<DROPFILES>()
            .write_unaligned(header);
    }
    for path in paths {
        for character in path.replace('/', "\\").encode_utf16().chain(Some(0)) {
            bytes.extend_from_slice(&character.to_le_bytes());
        }
    }
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes
}
fn text_bytes(text: &str) -> Vec<u8> {
    text.encode_utf16()
        .chain(Some(0))
        .flat_map(u16::to_le_bytes)
        .collect()
}

fn publish(api: &dyn ClipboardApi, buffers: &mut [(u32, Buffer<'_>)]) -> Result<()> {
    let owner = Owner(api, api.create_owner()?);
    let session = Session::new(api, owner.1)?;
    api.empty()?;
    for (format, buffer) in buffers {
        if let Err(error) = buffer.publish(*format) {
            // Do not expose paths with a missing/incorrect Cut effect on partial success.
            if let Err(cleanup) = api.empty() {
                return Err(CommandError::SystemError(format!("{error}; {cleanup}")));
            }
            return Err(error);
        }
    }
    session.close()
}

fn set_files_with(api: &dyn ClipboardApi, paths: &[String], cut: bool) -> Result<()> {
    if paths.is_empty() {
        return publish(api, &mut []);
    }
    // Prepare both formats before changing the previous clipboard contents.
    let format = api.effect_format()?;
    let effect = if cut { 2_u32 } else { 1_u32 };
    let effect = Buffer::new(api, &effect.to_le_bytes())?;
    let files = Buffer::new(api, &file_bytes(paths))?;
    // No usable paths are published until the Copy/Cut intent has been accepted.
    publish(api, &mut [(format, effect), (CF_HDROP.0 as u32, files)])
}
fn set_text_with(api: &dyn ClipboardApi, text: &str) -> Result<()> {
    let text = Buffer::new(api, &text_bytes(text))?;
    publish(api, &mut [(CF_UNICODETEXT.0 as u32, text)])
}

pub(super) fn set_files(paths: &[String], cut: bool) -> Result<()> {
    set_files_with(&NativeApi, paths, cut)
}
pub(super) fn set_text(text: &str) -> Result<()> {
    set_text_with(&NativeApi, text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::{BTreeMap, BTreeSet};

    struct State {
        memory: BTreeMap<usize, Box<[u8]>>,
        clipboard: BTreeMap<u32, usize>,
        locked: BTreeSet<usize>,
        freed: BTreeSet<usize>,
        calls: BTreeMap<&'static str, usize>,
        events: Vec<&'static str>,
        next: usize,
        owner: bool,
        open: bool,
        previous_contents: bool,
    }
    struct FakeApi {
        state: RefCell<State>,
        failure: Option<(&'static str, usize)>,
    }
    impl FakeApi {
        fn new(failure: Option<(&'static str, usize)>) -> Self {
            Self {
                failure,
                state: RefCell::new(State {
                    memory: BTreeMap::new(),
                    clipboard: BTreeMap::new(),
                    locked: BTreeSet::new(),
                    freed: BTreeSet::new(),
                    calls: BTreeMap::new(),
                    events: Vec::new(),
                    next: 1,
                    owner: false,
                    open: false,
                    previous_contents: true,
                }),
            }
        }
        fn step(&self, name: &'static str) -> Result<()> {
            let mut state = self.state.borrow_mut();
            state.events.push(name);
            let count = state.calls.entry(name).or_default();
            *count += 1;
            if self.failure == Some((name, *count)) {
                return Err(error("Injected clipboard failure", name));
            }
            Ok(())
        }
        fn assert_released(&self) {
            let state = self.state.borrow();
            assert!(!state.open && !state.owner);
            assert!(state.locked.is_empty());
            let system_owned: BTreeSet<_> = state.clipboard.values().copied().collect();
            assert_eq!(
                state.memory.keys().copied().collect::<BTreeSet<_>>(),
                system_owned
            );
            assert!(state.freed.is_disjoint(&system_owned));
        }
    }
    impl ClipboardApi for FakeApi {
        fn allocate(&self, size: usize) -> Result<HGLOBAL> {
            self.step("allocate")?;
            let mut state = self.state.borrow_mut();
            let handle = state.next;
            state.next += 1;
            state
                .memory
                .insert(handle, vec![0; size].into_boxed_slice());
            Ok(HGLOBAL(handle as *mut _))
        }
        fn lock(&self, handle: HGLOBAL) -> Result<*mut u8> {
            self.step("lock")?;
            let mut state = self.state.borrow_mut();
            assert!(state.locked.insert(handle.0 as usize));
            Ok(state
                .memory
                .get_mut(&(handle.0 as usize))
                .unwrap()
                .as_mut_ptr())
        }
        fn unlock(&self, handle: HGLOBAL) {
            assert!(self.state.borrow_mut().locked.remove(&(handle.0 as usize)));
        }
        fn free(&self, handle: HGLOBAL) {
            let mut state = self.state.borrow_mut();
            let handle = handle.0 as usize;
            assert!(!state.locked.contains(&handle));
            assert!(!state.clipboard.values().any(|owned| *owned == handle));
            assert!(state.freed.insert(handle), "double free");
            assert!(state.memory.remove(&handle).is_some());
        }
        fn create_owner(&self) -> Result<HWND> {
            self.step("owner")?;
            self.state.borrow_mut().owner = true;
            Ok(HWND(1_usize as *mut _))
        }
        fn destroy_owner(&self, _: HWND) {
            let mut state = self.state.borrow_mut();
            assert!(
                !state.open,
                "clipboard must close before its owner is destroyed"
            );
            assert!(state.owner);
            state.owner = false;
            state.events.push("destroy");
        }
        fn open(&self, owner: HWND) -> Result<()> {
            self.step("open")?;
            assert!(!owner.is_invalid());
            self.state.borrow_mut().open = true;
            Ok(())
        }
        fn empty(&self) -> Result<()> {
            self.step("empty")?;
            let mut state = self.state.borrow_mut();
            assert!(state.open && state.owner);
            let published: Vec<_> = state.clipboard.values().copied().collect();
            for handle in published {
                state.memory.remove(&handle).unwrap();
            }
            state.clipboard.clear();
            state.previous_contents = false;
            Ok(())
        }
        fn publish(&self, format: u32, handle: HGLOBAL) -> Result<()> {
            self.step("publish")?;
            let mut state = self.state.borrow_mut();
            assert!(state.open && state.owner);
            let handle = handle.0 as usize;
            assert!(!state.locked.contains(&handle));
            assert!(state.memory.contains_key(&handle));
            state.clipboard.insert(format, handle);
            Ok(())
        }
        fn close(&self) -> Result<()> {
            self.step("close")?;
            let mut state = self.state.borrow_mut();
            assert!(state.open);
            state.open = false;
            Ok(())
        }
        fn effect_format(&self) -> Result<u32> {
            self.step("register")?;
            Ok(0xC000)
        }
    }

    #[test]
    fn file_publication_failures_release_local_allocations_and_preserve_previous_data_when_possible(
    ) {
        let paths = vec![r"C:\Fichier école.txt".to_string()];
        for cut in [false, true] {
            for failure in [
                ("register", 1),
                ("allocate", 1),
                ("lock", 1),
                ("allocate", 2),
                ("lock", 2),
                ("owner", 1),
                ("open", 1),
                ("empty", 1),
                ("publish", 1),
                ("publish", 2),
                ("close", 1),
            ] {
                let api = FakeApi::new(Some(failure));
                assert!(set_files_with(&api, &paths, cut).is_err(), "{failure:?}");
                api.assert_released();
                let state = api.state.borrow();
                if matches!(
                    failure.0,
                    "register" | "allocate" | "lock" | "owner" | "open" | "empty"
                ) {
                    assert!(state.previous_contents, "{failure:?}");
                }
                if failure.0 == "publish" {
                    assert!(state.clipboard.is_empty());
                }
            }
        }
    }

    #[test]
    fn text_publication_failures_release_buffers_and_close_before_destroying_owner() {
        for failure in [
            ("allocate", 1),
            ("lock", 1),
            ("owner", 1),
            ("open", 1),
            ("empty", 1),
            ("publish", 1),
            ("close", 1),
        ] {
            let api = FakeApi::new(Some(failure));
            assert!(set_text_with(&api, "Texte école 🦀").is_err());
            api.assert_released();
        }
    }

    #[test]
    fn successful_copy_and_cut_transfer_both_unlocked_formats_to_system_ownership() {
        for cut in [false, true] {
            let api = FakeApi::new(None);
            set_files_with(&api, &[r"C:\Fichier école.txt".to_string()], cut).unwrap();
            api.assert_released();
            let state = api.state.borrow();
            assert_eq!(state.clipboard.len(), 2);
            let handle = state.clipboard[&0xC000];
            assert_eq!(
                &*state.memory[&handle],
                &(if cut { 2_u32 } else { 1_u32 }).to_le_bytes()
            );
            assert_eq!(state.events[state.events.len() - 2..], ["close", "destroy"]);
            assert!(state.freed.is_empty());
        }
    }

    #[test]
    fn clearing_file_clipboard_allocates_and_publishes_nothing() {
        let api = FakeApi::new(None);
        set_files_with(&api, &[], true).unwrap();
        api.assert_released();
        let state = api.state.borrow();
        assert!(!state.previous_contents);
        assert_eq!(state.calls.get("allocate"), None);
        assert_eq!(state.calls.get("publish"), None);
    }

    #[test]
    fn text_preserves_unicode_and_final_terminator() {
        let api = FakeApi::new(None);
        set_text_with(&api, "Texte école 🦀").unwrap();
        api.assert_released();
        let state = api.state.borrow();
        let bytes = &state.memory[&state.clipboard[&(CF_UNICODETEXT.0 as u32)]];
        let characters: Vec<_> = bytes
            .chunks_exact(2)
            .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
            .collect();
        assert_eq!(characters.last(), Some(&0));
        assert_eq!(
            String::from_utf16(&characters[..characters.len() - 1]).unwrap(),
            "Texte école 🦀"
        );
    }

    #[test]
    fn native_dropfiles_buffer_preserves_unicode_unc_and_normalized_paths() {
        use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};
        let paths = vec![
            r"C:/Dossier école/Fichier 🦀.txt".into(),
            r"\\serveur\partage\Deuxième fichier.txt".into(),
        ];
        let buffer = Buffer::new(&NativeApi, &file_bytes(&paths)).unwrap();
        let drop = HDROP(buffer.handle.unwrap().0);
        assert_eq!(unsafe { DragQueryFileW(drop, u32::MAX, None) }, 2);
        for (index, path) in paths.iter().enumerate() {
            let length = unsafe { DragQueryFileW(drop, index as u32, None) };
            let mut text = vec![0; length as usize + 1];
            assert_eq!(
                unsafe { DragQueryFileW(drop, index as u32, Some(&mut text)) },
                length
            );
            assert_eq!(
                String::from_utf16(&text[..length as usize]).unwrap(),
                path.replace('/', "\\")
            );
        }
    }

    #[test]
    fn native_owner_is_invisible_and_destroyed_on_its_creating_thread() {
        use windows::Win32::UI::WindowsAndMessaging::{
            GetWindowThreadProcessId, IsWindow, IsWindowVisible,
        };
        let owner = Owner(&NativeApi, NativeApi.create_owner().unwrap());
        let handle = owner.1;
        assert!(unsafe { IsWindow(Some(handle)) }.as_bool());
        assert!(!unsafe { IsWindowVisible(handle) }.as_bool());
        assert_eq!(unsafe { GetWindowThreadProcessId(handle, None) }, unsafe {
            windows::Win32::System::Threading::GetCurrentThreadId()
        });
        drop(owner);
        assert!(!unsafe { IsWindow(Some(handle)) }.as_bool());
    }
}
