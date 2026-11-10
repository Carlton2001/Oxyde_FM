//! The library as an application uses it: loaded, asked for a dialog, the dialog set
//! up and shown. A fake launcher (`examples/fake_launcher.rs`) stands for Oxyde, so
//! nothing shows on screen; the system's dialog is never shown here.
#![cfg(windows)]
use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use windows::core::{implement, w, Interface, Ref, GUID, HRESULT, HSTRING, PCSTR, PCWSTR};
use windows::Win32::Foundation::{ERROR_CANCELLED, E_NOTIMPL, S_FALSE};
use windows::Win32::System::Com::{CoInitializeEx, CoTaskMemFree, IClassFactory, COINIT_APARTMENTTHREADED};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::{
    IFileDialog, IFileDialog2, IFileDialogCustomize, IFileDialogEvents, IFileDialogEvents_Impl, IFileOpenDialog, IFileSaveDialog, IShellItem,
    SHCreateItemFromParsingName, FDE_OVERWRITE_RESPONSE, FDE_SHAREVIOLATION_RESPONSE, FOS_ALLOWMULTISELECT, FOS_PICKFOLDERS, SIGDN_FILESYSPATH,
};

const OPEN: GUID = GUID::from_u128(0xDC1C5A9C_E88A_4DDE_A5A1_60F82A20AEF7);
const SAVE: GUID = GUID::from_u128(0xC0B4E2F3_BA21_4773_8DBA_335EC946EB8B);

type GetClassObject = unsafe extern "system" fn(*const GUID, *const GUID, *mut *mut c_void) -> HRESULT;

/// The library and the fake launcher side by side, as Oxyde installs them.
struct Installed {
    folder: PathBuf,
    classes: GetClassObject,
}

impl Installed {
    fn new() -> Self {
        // target/debug/deps/dialog-….exe: the library and the examples are one folder up.
        let built = std::env::current_exe().unwrap().parent().unwrap().parent().unwrap().to_owned();
        let folder = std::env::temp_dir().join(format!("oxyde-file-dialog-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::copy(built.join("oxyde_file_dialog.dll"), folder.join("oxyde_file_dialog.dll")).unwrap();
        std::fs::copy(built.join("examples").join("fake_launcher.exe"), folder.join("oxyde-folder-launcher.exe")).unwrap();
        unsafe {
            let library = LoadLibraryW(&HSTRING::from(folder.join("oxyde_file_dialog.dll").as_os_str())).unwrap();
            let classes = GetProcAddress(library, PCSTR(c"DllGetClassObject".as_ptr().cast())).unwrap();
            Self { folder, classes: std::mem::transmute::<unsafe extern "system" fn() -> isize, GetClassObject>(classes) }
        }
    }

    fn dialog<T: Interface>(&self, class: &GUID) -> T {
        unsafe {
            let mut factory = std::ptr::null_mut();
            (self.classes)(class, &IClassFactory::IID, &mut factory).ok().unwrap();
            IClassFactory::from_raw(factory).CreateInstance(None).unwrap()
        }
    }

    /// What the launcher answers from now on, one answer for each time it is run.
    fn answers(&self, answers: &[&str]) {
        let _ = std::fs::remove_file(self.requests());
        std::env::set_var("OXYDE_FAKE_REQUESTS", self.requests());
        std::env::set_var("OXYDE_FAKE_ANSWERS", answers.join("\n--\n"));
    }

    fn requests(&self) -> PathBuf {
        self.folder.join("requests.txt")
    }

    /// What the launcher was asked, each time it was run.
    fn asked(&self) -> Vec<String> {
        let asked = std::fs::read_to_string(self.requests()).unwrap_or_default();
        asked.split("\n--\n").filter(|request| !request.is_empty()).map(str::to_owned).collect()
    }
}

fn path(item: &IShellItem) -> String {
    unsafe {
        let name = item.GetDisplayName(SIGDN_FILESYSPATH).unwrap();
        let path = name.to_string().unwrap();
        CoTaskMemFree(Some(name.0.cast()));
        path
    }
}

fn item(path: &Path) -> IShellItem {
    unsafe { SHCreateItemFromParsingName(&HSTRING::from(path.as_os_str()), None).unwrap() }
}

/// A listener that refuses the first choices it is told of.
#[implement(IFileDialogEvents)]
struct Listener {
    refusals: Cell<usize>,
    told: RefCell<Vec<String>>,
}

impl IFileDialogEvents_Impl for Listener_Impl {
    fn OnFileOk(&self, dialog: Ref<IFileDialog>) -> windows::core::Result<()> {
        let chosen = unsafe { dialog.unwrap().GetResult() }?;
        self.told.borrow_mut().push(path(&chosen));
        if self.refusals.get() > 0 {
            self.refusals.set(self.refusals.get() - 1);
            return Err(S_FALSE.into());
        }
        Ok(())
    }
    fn OnFolderChanging(&self, _: Ref<IFileDialog>, _: Ref<IShellItem>) -> windows::core::Result<()> { Ok(()) }
    fn OnFolderChange(&self, _: Ref<IFileDialog>) -> windows::core::Result<()> { Ok(()) }
    fn OnSelectionChange(&self, _: Ref<IFileDialog>) -> windows::core::Result<()> { Ok(()) }
    fn OnShareViolation(&self, _: Ref<IFileDialog>, _: Ref<IShellItem>) -> windows::core::Result<FDE_SHAREVIOLATION_RESPONSE> { Err(E_NOTIMPL.into()) }
    fn OnTypeChange(&self, _: Ref<IFileDialog>) -> windows::core::Result<()> { Ok(()) }
    fn OnOverwrite(&self, _: Ref<IFileDialog>, _: Ref<IShellItem>) -> windows::core::Result<FDE_OVERWRITE_RESPONSE> { Err(E_NOTIMPL.into()) }
}

fn opens_what_oxyde_answers(installed: &Installed, folder: &Path) {
    let (first, second) = (folder.join("a.txt"), folder.join("b.md"));
    std::fs::write(&first, b"").unwrap();
    std::fs::write(&second, b"").unwrap();
    installed.answers(&[&format!("ok\nfilter\t2\npath\t{}\npath\t{}\n", first.display(), second.display())]);
    let dialog: IFileOpenDialog = installed.dialog(&OPEN);
    unsafe {
        let types = [
            COMDLG_FILTERSPEC { pszName: w!("Text"), pszSpec: w!("*.txt;*.md") },
            COMDLG_FILTERSPEC { pszName: w!("All files"), pszSpec: w!("*.*") },
        ];
        dialog.SetFileTypes(&types).unwrap();
        dialog.SetTitle(w!("Choose notes")).unwrap();
        dialog.SetOkButtonLabel(w!("&Take")).unwrap();
        dialog.SetOptions(dialog.GetOptions().unwrap() | FOS_ALLOWMULTISELECT).unwrap();
        dialog.SetFolder(&item(folder)).unwrap();
        dialog.Show(None).unwrap();
        let chosen = dialog.GetResults().unwrap();
        assert_eq!(chosen.GetCount().unwrap(), 2);
        assert_eq!(path(&chosen.GetItemAt(0).unwrap()), first.to_str().unwrap());
        assert_eq!(path(&chosen.GetItemAt(1).unwrap()), second.to_str().unwrap());
        assert_eq!(path(&dialog.GetResult().unwrap()), first.to_str().unwrap());
        assert_eq!(path(&dialog.GetFolder().unwrap()), folder.to_str().unwrap());
        assert_eq!(dialog.GetFileTypeIndex().unwrap(), 2);
        let name = dialog.GetFileName().unwrap();
        assert_eq!(name.to_string().unwrap(), "a.txt");
        CoTaskMemFree(Some(name.0.cast()));
    }
    let expected = format!(
        "purpose\topen\nmultiple\t1\ntitle\tChoose notes\naccept\t&Take\nfolder\t{}\nfilter\tText\t*.txt;*.md\nfilter\tAll files\t*.*\nfilter-index\t1\n",
        folder.display()
    );
    assert_eq!(installed.asked(), [expected]);
}

fn a_cancelled_dialog_says_so(installed: &Installed) {
    installed.answers(&["cancel\n"]);
    let dialog: IFileOpenDialog = installed.dialog(&OPEN);
    let shown = unsafe { dialog.Show(None) };
    assert_eq!(shown.unwrap_err().code(), HRESULT::from_win32(ERROR_CANCELLED.0));
    assert!(installed.asked()[0].starts_with("purpose\topen\n"));
}

fn folders_are_asked_for_as_folders(installed: &Installed, folder: &Path) {
    installed.answers(&[&format!("ok\npath\t{}\n", folder.display())]);
    let dialog: IFileOpenDialog = installed.dialog(&OPEN);
    unsafe {
        dialog.SetOptions(dialog.GetOptions().unwrap() | FOS_PICKFOLDERS).unwrap();
        dialog.Show(None).unwrap();
        assert_eq!(path(&dialog.GetResult().unwrap()), folder.to_str().unwrap());
    }
    assert!(installed.asked()[0].starts_with("purpose\tfolder\n"));
}

fn saves_under_a_name_that_is_still_free(installed: &Installed, folder: &Path) {
    let saved = folder.join("new report.txt");
    installed.answers(&[&format!("ok\nfilter\t1\npath\t{}\n", saved.display())]);
    let dialog: IFileSaveDialog = installed.dialog(&SAVE);
    unsafe {
        dialog.SetFileTypes(&[COMDLG_FILTERSPEC { pszName: w!("Text"), pszSpec: w!("*.txt") }]).unwrap();
        dialog.SetFileName(w!("report")).unwrap();
        dialog.SetDefaultExtension(w!("txt")).unwrap();
        dialog.SetFolder(&item(folder)).unwrap();
        dialog.Show(None).unwrap();
        assert_eq!(path(&dialog.GetResult().unwrap()), saved.to_str().unwrap());
    }
    let expected = format!("purpose\tsave\nname\treport\nfolder\t{}\nextension\ttxt\nfilter\tText\t*.txt\nfilter-index\t1\n", folder.display());
    assert_eq!(installed.asked(), [expected]);
}

fn a_refused_choice_is_asked_again(installed: &Installed, folder: &Path) {
    let (first, second) = (folder.join("a.txt"), folder.join("b.md"));
    installed.answers(&[&format!("ok\npath\t{}\n", first.display()), &format!("ok\npath\t{}\n", second.display())]);
    let dialog: IFileOpenDialog = installed.dialog(&OPEN);
    let listener: IFileDialogEvents = Listener { refusals: Cell::new(1), told: RefCell::default() }.into();
    unsafe {
        let cookie = dialog.Advise(&listener).unwrap();
        dialog.Show(None).unwrap();
        assert_eq!(path(&dialog.GetResult().unwrap()), second.to_str().unwrap());
        dialog.Unadvise(cookie).unwrap();
    }
    assert_eq!(installed.asked().len(), 2);
}

fn the_other_interfaces_are_there(installed: &Installed) {
    let dialog: IFileOpenDialog = installed.dialog(&OPEN);
    let second: IFileDialog2 = dialog.cast().unwrap();
    assert_eq!(second.cast::<IFileOpenDialog>().unwrap(), dialog);
    let controls: IFileDialogCustomize = dialog.cast().unwrap();
    assert_eq!(controls.cast::<IFileOpenDialog>().unwrap(), dialog);
    assert!(dialog.cast::<IFileSaveDialog>().is_err());
    unsafe { second.SetCancelButtonLabel(PCWSTR(w!("Leave").as_ptr())).unwrap() };
    let saving: IFileSaveDialog = installed.dialog(&SAVE);
    assert!(saving.cast::<IFileOpenDialog>().is_err());
    assert_eq!(saving.cast::<IFileDialog>().unwrap().cast::<IFileSaveDialog>().unwrap(), saving);
}

/// One test: the fake launcher is told what to answer through the environment, which
/// all the dialogs of the process share.
#[test]
fn the_dialog_applications_get_is_answered_by_the_launcher() {
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok().unwrap() };
    let installed = Installed::new();
    let folder = installed.folder.join("files");
    std::fs::create_dir_all(&folder).unwrap();
    let folder = folder.canonicalize().unwrap();
    let folder = PathBuf::from(folder.to_str().unwrap().trim_start_matches(r"\\?\"));
    opens_what_oxyde_answers(&installed, &folder);
    a_cancelled_dialog_says_so(&installed);
    folders_are_asked_for_as_folders(&installed, &folder);
    saves_under_a_name_that_is_still_free(&installed, &folder);
    a_refused_choice_is_asked_again(&installed, &folder);
    the_other_interfaces_are_there(&installed);
}
