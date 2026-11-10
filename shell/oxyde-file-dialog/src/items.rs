//! Paths as the Shell items applications hand to a dialog and read from it.
use windows::core::{Interface, Result, HSTRING, PCWSTR, PWSTR};
use windows::Win32::Foundation::{E_FAIL, MAX_PATH};
use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemAlloc, CoTaskMemFree, IPersistFile, CLSCTX_INPROC_SERVER, STGM_READ};
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{
    IShellItem, IShellItemArray, IShellLinkW, SHCreateItemFromIDList, SHCreateItemFromParsingName, SHCreateShellItemArrayFromIDLists,
    SHGetIDListFromObject, SHSimpleIDListFromPath, ShellLink, SIGDN_FILESYSPATH, SIGDN_PARENTRELATIVEPARSING,
};

/// Text the caller frees with `CoTaskMemFree`, as COM returns strings.
pub(crate) fn task_text(text: &str) -> PWSTR {
    let wide: Vec<u16> = text.encode_utf16().chain([0]).collect();
    unsafe {
        let memory = CoTaskMemAlloc(wide.len() * 2).cast::<u16>();
        if !memory.is_null() {
            std::ptr::copy_nonoverlapping(wide.as_ptr(), memory, wide.len());
        }
        PWSTR(memory)
    }
}

fn taken(text: Result<PWSTR>) -> Option<String> {
    let text = text.ok()?;
    unsafe {
        let read = text.to_string().ok();
        CoTaskMemFree(Some(text.0.cast()));
        read
    }
}

/// The path of a file or folder; None for an item that is not on a disk.
pub(crate) fn path(item: &IShellItem) -> Option<String> {
    taken(unsafe { item.GetDisplayName(SIGDN_FILESYSPATH) }).filter(|path| !path.is_empty())
}

/// The name of an item in its folder.
pub(crate) fn name(item: &IShellItem) -> Option<String> {
    taken(unsafe { item.GetDisplayName(SIGDN_PARENTRELATIVEPARSING) }).filter(|name| !name.is_empty())
}

struct IdList(*mut ITEMIDLIST);
impl Drop for IdList {
    fn drop(&mut self) { unsafe { CoTaskMemFree(Some(self.0.cast())) } }
}

/// The identifier list of `path`; a file still to be saved has none of its own, and
/// gets one that only names it.
fn id_list(path: &str) -> Result<IdList> {
    let name = HSTRING::from(path);
    unsafe {
        if let Ok(item) = SHCreateItemFromParsingName::<_, _, IShellItem>(&name, None) {
            if let Ok(list) = SHGetIDListFromObject(&item) {
                return Ok(IdList(list));
            }
        }
        let list = SHSimpleIDListFromPath(&name);
        if list.is_null() { Err(E_FAIL.into()) } else { Ok(IdList(list)) }
    }
}

pub(crate) fn item(path: &str) -> Result<IShellItem> {
    let name = HSTRING::from(path);
    unsafe {
        SHCreateItemFromParsingName(&name, None).or_else(|_| {
            let list = id_list(path)?;
            SHCreateItemFromIDList(list.0)
        })
    }
}

pub(crate) fn array(paths: &[String]) -> Result<IShellItemArray> {
    let lists = paths.iter().map(|path| id_list(path)).collect::<Result<Vec<_>>>()?;
    let pointers: Vec<*const ITEMIDLIST> = lists.iter().map(|list| list.0.cast_const()).collect();
    if pointers.is_empty() {
        return Err(E_FAIL.into());
    }
    unsafe { SHCreateShellItemArrayFromIDLists(&pointers) }
}

/// The file a shortcut leads to, which a dialog opens in its place unless the
/// application asks for the shortcut itself; None for anything else.
pub(crate) fn shortcut_target(path: &str) -> Option<String> {
    if !path.to_ascii_lowercase().ends_with(".lnk") {
        return None;
    }
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
        link.cast::<IPersistFile>().ok()?.Load(PCWSTR(HSTRING::from(path).as_ptr()), STGM_READ).ok()?;
        let mut target = [0u16; MAX_PATH as usize * 4];
        link.GetPath(&mut target, std::ptr::null_mut(), 0).ok()?;
        let length = target.iter().position(|unit| *unit == 0)?;
        let target = String::from_utf16(&target[..length]).ok()?;
        std::path::Path::new(&target).is_file().then_some(target)
    }
}
