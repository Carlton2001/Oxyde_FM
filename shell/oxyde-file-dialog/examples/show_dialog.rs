//! Shows an Open or Save dialog as an application would, through the classes the
//! system registers, so the dialog is Oxyde's when its option is checked and Windows'
//! otherwise; prints what the dialog answered once it closes. For checks by hand:
//!
//! `cargo run --offline --manifest-path shell/oxyde-file-dialog/Cargo.toml --target-dir target --example show_dialog -- [CASE…]`
//!
//! The cases work in `%TEMP%\oxyde-dialog-check`, made at each run with a few files.
#[cfg(not(windows))]
fn main() {
    eprintln!("Windows only: shows the Open and Save dialogs of Windows.");
}

#[cfg(windows)]
fn main() {
    windows_dialog::main();
}

#[cfg(windows)]
mod windows_dialog {
    use std::cell::Cell;
    use std::path::{Path, PathBuf};
    use windows::core::{implement, w, Interface, Ref, GUID, HRESULT, HSTRING, PCWSTR};
    use windows::Win32::Foundation::{ERROR_CANCELLED, E_NOTIMPL, HWND, S_FALSE};
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED};
    use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
    use windows::Win32::UI::Shell::{
        FileOpenDialog, FileSaveDialog, IFileDialog, IFileDialog2, IFileDialogCustomize, IFileDialogEvents, IFileDialogEvents_Impl,
        IFileOpenDialog, IFileSaveDialog, IShellItem, SHCreateItemFromParsingName, CDCS_INACTIVE, CDCS_VISIBLE, FDAP_BOTTOM, FDAP_TOP,
        FDE_OVERWRITE_RESPONSE, FDE_SHAREVIOLATION_RESPONSE, FILEOPENDIALOGOPTIONS, FOS_ALLNONSTORAGEITEMS, FOS_ALLOWMULTISELECT,
        FOS_FORCESHOWHIDDEN, FOS_HIDEPINNEDPLACES, FOS_OKBUTTONNEEDSINTERACTION, FOS_OVERWRITEPROMPT, FOS_PICKFOLDERS, FOS_STRICTFILETYPES,
        SIGDN_FILESYSPATH,
    };
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

    const CASES: &[(&str, &str)] = &[
        ("open", "Open, one file, in the check folder"),
        ("multiple", "Open, several files"),
        ("folder", "Open with FOS_PICKFOLDERS: select folders"),
        ("filters", "Open with three type filters, the second shown first"),
        ("controls", "Open with a combo box, radio buttons, check buttons, an edit box, a note, a disabled control and the open drop-down"),
        ("refuse", "Open whose listener refuses the first choice: Oxyde asks again"),
        ("options", "Open with labels, hidden files, needs interaction, hidden pins, added places and a default folder"),
        ("client-a", "Open remembered under client GUID A"),
        ("client-b", "Open remembered under client GUID B"),
        ("save", "Save with a suggested name, a default extension and type filters"),
        ("save-again", "Save of an existing file (SetSaveAsItem)"),
        ("save-strict", "Save with FOS_STRICTFILETYPES, without FOS_OVERWRITEPROMPT"),
        ("many-controls", "Save with type filters, six combo boxes and three check buttons"),
        ("push-button", "Open with a push button: the system's dialog shows"),
        ("navigation-root", "Open with SetNavigationRoot: the system's dialog shows"),
        ("non-storage", "Open with FOS_ALLNONSTORAGEITEMS: the system's dialog shows"),
    ];
    /// The cases `all` runs: those Oxyde answers.
    const ALL: &[&str] = &["open", "multiple", "folder", "filters", "controls", "refuse", "options", "save", "save-again", "save-strict", "many-controls"];
    const CLIENT_A: GUID = GUID::from_u128(0x6f0d3b52_4c1e_4a8e_9a3b_1f6e2c7d8a01);
    const CLIENT_B: GUID = GUID::from_u128(0x6f0d3b52_4c1e_4a8e_9a3b_1f6e2c7d8a02);

    fn usage() {
        eprintln!("Usage: show_dialog [CASE...]\nShows each case's dialog in turn and prints its answer.\n\nCases:");
        for (name, what) in CASES {
            eprintln!("  {name:<16}{what}");
        }
        eprintln!("  {:<16}every case Oxyde answers, one after the other", "all");
    }

    pub(super) fn main() {
        let mut cases: Vec<String> = std::env::args().skip(1).collect();
        if cases.iter().any(|case| case == "--help" || case == "-h") {
            usage();
            return;
        }
        if cases.is_empty() {
            cases.push("open".into());
        }
        if cases.iter().any(|case| case == "all") {
            cases = ALL.iter().map(|case| case.to_string()).collect();
        }
        if let Some(unknown) = cases.iter().find(|case| !CASES.iter().any(|(name, _)| name == case)) {
            eprintln!("Unknown case: {unknown}\n");
            usage();
            std::process::exit(2);
        }
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok().expect("COM") };
        let folder = fixture();
        // The terminal's window owns the dialog, as an application's window would.
        let owner = unsafe { GetForegroundWindow() };
        for case in &cases {
            let what = CASES.iter().find(|(name, _)| name == case).map(|(_, what)| *what).unwrap_or_default();
            println!("== {case}: {what}");
            if let Err(error) = unsafe { run(case, &folder, owner) } {
                println!("   failed: {error}");
            }
        }
    }

    /// `%TEMP%\oxyde-dialog-check`: files of several types, a read-only one, a hidden
    /// one and a subfolder.
    fn fixture() -> PathBuf {
        let folder = std::env::temp_dir().join("oxyde-dialog-check");
        std::fs::create_dir_all(folder.join("Subfolder")).expect("check folder");
        for name in ["notes.txt", "readme.md", "photo.png", "report.docx", "Subfolder\\inner.txt"] {
            let path = folder.join(name);
            if !path.exists() {
                std::fs::write(&path, b"Oxyde dialog check\r\n").expect("check file");
            }
        }
        let read_only = folder.join("read-only.txt");
        if !read_only.exists() {
            std::fs::write(&read_only, b"read only\r\n").expect("check file");
            let mut permissions = std::fs::metadata(&read_only).expect("check file").permissions();
            permissions.set_readonly(true);
            std::fs::set_permissions(&read_only, permissions).expect("read-only file");
        }
        let hidden = folder.join("hidden.txt");
        if !hidden.exists() {
            std::fs::write(&hidden, b"hidden\r\n").expect("check file");
            let _ = std::process::Command::new("attrib").arg("+h").arg(&hidden).status();
        }
        let folder = folder.canonicalize().expect("check folder");
        PathBuf::from(folder.to_string_lossy().trim_start_matches(r"\\?\"))
    }

    fn item(path: &Path) -> windows::core::Result<IShellItem> {
        unsafe { SHCreateItemFromParsingName(&HSTRING::from(path.as_os_str()), None) }
    }

    fn path(item: &IShellItem) -> String {
        unsafe {
            match item.GetDisplayName(SIGDN_FILESYSPATH) {
                Ok(name) => {
                    let path = name.to_string().unwrap_or_default();
                    CoTaskMemFree(Some(name.0.cast()));
                    path
                }
                Err(error) => format!("(no file system path: {error})"),
            }
        }
    }

    fn without(options: FILEOPENDIALOGOPTIONS, removed: FILEOPENDIALOGOPTIONS) -> FILEOPENDIALOGOPTIONS {
        FILEOPENDIALOGOPTIONS(options.0 & !removed.0)
    }

    const TYPES: [COMDLG_FILTERSPEC; 3] = [
        COMDLG_FILTERSPEC { pszName: w!("Text"), pszSpec: w!("*.txt;*.md") },
        COMDLG_FILTERSPEC { pszName: w!("Images"), pszSpec: w!("*.png;*.jpg") },
        COMDLG_FILTERSPEC { pszName: w!("All files"), pszSpec: w!("*.*") },
    ];

    /// A listener that refuses the first choice it is told of, and says what it was told.
    #[implement(IFileDialogEvents)]
    struct Listener {
        refusals: Cell<usize>,
    }

    impl IFileDialogEvents_Impl for Listener_Impl {
        fn OnFileOk(&self, dialog: Ref<IFileDialog>) -> windows::core::Result<()> {
            let chosen = unsafe { dialog.ok()?.GetResult() }.map(|item| path(&item)).unwrap_or_default();
            if self.refusals.get() > 0 {
                self.refusals.set(self.refusals.get() - 1);
                println!("   OnFileOk({chosen}): refused, the dialog asks again");
                return Err(S_FALSE.into());
            }
            println!("   OnFileOk({chosen}): accepted");
            Ok(())
        }
        fn OnFolderChanging(&self, _: Ref<IFileDialog>, _: Ref<IShellItem>) -> windows::core::Result<()> { Ok(()) }
        fn OnFolderChange(&self, _: Ref<IFileDialog>) -> windows::core::Result<()> { Ok(()) }
        fn OnSelectionChange(&self, _: Ref<IFileDialog>) -> windows::core::Result<()> { Ok(()) }
        fn OnShareViolation(&self, _: Ref<IFileDialog>, _: Ref<IShellItem>) -> windows::core::Result<FDE_SHAREVIOLATION_RESPONSE> { Err(E_NOTIMPL.into()) }
        fn OnTypeChange(&self, _: Ref<IFileDialog>) -> windows::core::Result<()> { Ok(()) }
        fn OnOverwrite(&self, _: Ref<IFileDialog>, _: Ref<IShellItem>) -> windows::core::Result<FDE_OVERWRITE_RESPONSE> { Err(E_NOTIMPL.into()) }
    }

    /// The application's controls: their ids, for reading them back.
    #[derive(Default)]
    struct Added {
        combos: Vec<u32>,
        checks: Vec<u32>,
        edits: Vec<u32>,
        drop_down: Option<u32>,
    }

    unsafe fn add_controls(controls: &IFileDialogCustomize) -> windows::core::Result<Added> {
        let mut added = Added::default();
        controls.StartVisualGroup(1, w!("&Encoding:"))?;
        controls.AddComboBox(2)?;
        controls.AddControlItem(2, 10, w!("UTF-8"))?;
        controls.AddControlItem(2, 11, w!("UTF-16"))?;
        controls.AddControlItem(2, 12, w!("ANSI"))?;
        controls.AddControlItem(2, 13, w!("Hidden item"))?;
        controls.SetControlItemState(2, 13, CDCS_INACTIVE)?;
        controls.SetSelectedControlItem(2, 11)?;
        controls.EndVisualGroup()?;
        added.combos.push(2);
        controls.StartVisualGroup(3, w!("Line endings"))?;
        controls.AddRadioButtonList(4)?;
        controls.AddControlItem(4, 20, w!("Windows (CRLF)"))?;
        controls.AddControlItem(4, 21, w!("Unix (LF)"))?;
        controls.SetSelectedControlItem(4, 20)?;
        controls.EndVisualGroup()?;
        added.combos.push(4);
        controls.AddCheckButton(5, w!("Open &read-only"), false)?;
        controls.AddCheckButton(6, w!("Keep a backup"), true)?;
        added.checks.extend([5, 6]);
        // A text before an edit box names it.
        controls.AddText(7, w!("Author:"))?;
        controls.AddEditBox(8, w!("Ada"))?;
        added.edits.push(8);
        // Shown but disabled.
        controls.AddCheckButton(9, w!("Locked by policy"), true)?;
        controls.SetControlState(9, CDCS_VISIBLE)?;
        added.checks.push(9);
        // A text naming nothing is a note.
        controls.AddText(14, w!("Only files signed by the team are accepted."))?;
        controls.EnableOpenDropDown(15)?;
        controls.AddControlItem(15, 30, w!("&Open"))?;
        controls.AddControlItem(15, 31, w!("Open &read-only"))?;
        controls.AddControlItem(15, 32, w!("Open a &copy"))?;
        added.drop_down = Some(15);
        Ok(added)
    }

    unsafe fn add_many_controls(controls: &IFileDialogCustomize) -> windows::core::Result<Added> {
        let mut added = Added::default();
        let menus: [(&str, &[&str]); 6] = [
            ("Encoding", &["UTF-8", "Latin-1"]),
            ("Line endings", &["Unix (LF)", "Windows (CRLF)"]),
            ("Quality", &["High", "Medium", "Low"]),
            ("Color profile", &["sRGB", "Display P3"]),
            ("Layers", &["Keep", "Flatten"]),
            ("Format version", &["2.0", "1.x compatible"]),
        ];
        let mut id = 100;
        for (label, items) in menus {
            controls.StartVisualGroup(id, &HSTRING::from(label))?;
            let combo = id + 1;
            controls.AddComboBox(combo)?;
            for (position, text) in items.iter().enumerate() {
                controls.AddControlItem(combo, position as u32, &HSTRING::from(*text))?;
            }
            controls.SetSelectedControlItem(combo, 0)?;
            controls.EndVisualGroup()?;
            added.combos.push(combo);
            id += 10;
        }
        for (offset, (label, checked)) in [("Keep metadata", true), ("Make a backup", false), ("Open after saving", false)].into_iter().enumerate() {
            let check = 200 + offset as u32;
            controls.AddCheckButton(check, &HSTRING::from(label), checked)?;
            added.checks.push(check);
        }
        Ok(added)
    }

    unsafe fn print_controls(controls: &IFileDialogCustomize, added: &Added) {
        for combo in &added.combos {
            println!("   menu {combo}: item {:?}", controls.GetSelectedControlItem(*combo));
        }
        for check in &added.checks {
            println!("   checkbox {check}: {:?}", controls.GetCheckButtonState(*check).map(|checked| checked.as_bool()));
        }
        for edit in &added.edits {
            match controls.GetEditBoxText(*edit) {
                Ok(text) if !text.is_null() => {
                    println!("   text field {edit}: {:?}", PCWSTR(text).to_string().unwrap_or_default());
                    CoTaskMemFree(Some(text.cast()));
                }
                other => println!("   text field {edit}: {:?}", other.map(|_| ())),
            }
        }
        if let Some(drop_down) = added.drop_down {
            println!("   open drop-down: item {:?}", controls.GetSelectedControlItem(drop_down));
        }
    }

    unsafe fn run(case: &str, folder: &Path, owner: HWND) -> windows::core::Result<()> {
        let saving = case.starts_with("save") || case == "many-controls";
        let dialog: IFileDialog = if saving {
            CoCreateInstance::<_, IFileSaveDialog>(&FileSaveDialog, None, CLSCTX_INPROC_SERVER)?.cast()?
        } else {
            CoCreateInstance::<_, IFileOpenDialog>(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?.cast()?
        };
        let controls: IFileDialogCustomize = dialog.cast()?;
        let second: IFileDialog2 = dialog.cast()?;
        let options = dialog.GetOptions()?;
        let mut added = Added::default();
        let mut listener = None;
        dialog.SetTitle(&HSTRING::from(format!("Oxyde check: {case}")))?;
        if !case.starts_with("client") && case != "options" {
            dialog.SetFolder(&item(folder)?)?;
        }
        match case {
            "open" => {}
            "multiple" => {
                dialog.SetOptions(options | FOS_ALLOWMULTISELECT)?;
                dialog.SetFileTypes(&TYPES)?;
            }
            "folder" => dialog.SetOptions(options | FOS_PICKFOLDERS | FOS_ALLOWMULTISELECT)?,
            "filters" => {
                dialog.SetFileTypes(&TYPES)?;
                dialog.SetFileTypeIndex(2)?;
            }
            "controls" => added = add_controls(&controls)?,
            "refuse" => {
                let events: IFileDialogEvents = Listener { refusals: Cell::new(1) }.into();
                listener = Some(dialog.Advise(&events)?);
            }
            "options" => {
                dialog.SetOptions(options | FOS_FORCESHOWHIDDEN | FOS_OKBUTTONNEEDSINTERACTION | FOS_HIDEPINNEDPLACES)?;
                dialog.SetOkButtonLabel(w!("&Take"))?;
                dialog.SetFileNameLabel(w!("&Document:"))?;
                second.SetCancelButtonLabel(w!("&Leave"))?;
                dialog.AddPlace(&item(folder)?, FDAP_TOP)?;
                dialog.AddPlace(&item(&folder.join("Subfolder"))?, FDAP_BOTTOM)?;
                dialog.SetDefaultFolder(&item(folder)?)?;
                dialog.SetClientGuid(&GUID::from_u128(0x6f0d3b52_4c1e_4a8e_9a3b_1f6e2c7d8a03))?;
            }
            "client-a" => dialog.SetClientGuid(&CLIENT_A)?,
            "client-b" => dialog.SetClientGuid(&CLIENT_B)?,
            "save" => {
                dialog.SetFileTypes(&TYPES)?;
                dialog.SetFileName(w!("New notes"))?;
                dialog.SetDefaultExtension(w!("txt"))?;
            }
            "save-again" => {
                let save: IFileSaveDialog = dialog.cast()?;
                save.SetSaveAsItem(&item(&folder.join("notes.txt"))?)?;
                dialog.SetFileTypes(&TYPES)?;
            }
            "save-strict" => {
                dialog.SetOptions(without(options, FOS_OVERWRITEPROMPT) | FOS_STRICTFILETYPES)?;
                dialog.SetFileTypes(&TYPES)?;
                dialog.SetFileTypeIndex(2)?;
                dialog.SetFileName(w!("picture"))?;
                dialog.SetDefaultExtension(w!("png"))?;
                dialog.SetFileNameLabel(w!("Export &as:"))?;
                dialog.SetOkButtonLabel(w!("&Export"))?;
            }
            "many-controls" => {
                dialog.SetFileTypes(&TYPES)?;
                dialog.SetFileTypeIndex(2)?;
                dialog.SetFileName(w!("Picture.png"))?;
                added = add_many_controls(&controls)?;
            }
            "push-button" => controls.AddPushButton(40, w!("Preview"))?,
            "navigation-root" => second.SetNavigationRoot(&item(folder)?)?,
            "non-storage" => dialog.SetOptions(options | FOS_ALLNONSTORAGEITEMS)?,
            _ => unreachable!("known case"),
        }
        let shown = dialog.Show(Some(owner));
        if let Some(cookie) = listener {
            dialog.Unadvise(cookie)?;
        }
        match shown {
            Err(error) if error.code() == HRESULT::from_win32(ERROR_CANCELLED.0) => println!("   cancelled"),
            Err(error) => return Err(error),
            Ok(()) => {
                if let Ok(open) = dialog.cast::<IFileOpenDialog>() {
                    let chosen = open.GetResults()?;
                    for index in 0..chosen.GetCount()? {
                        println!("   chosen: {}", path(&chosen.GetItemAt(index)?));
                    }
                } else {
                    println!("   saved as: {}", path(&dialog.GetResult()?));
                }
                println!("   type filter: {:?}", dialog.GetFileTypeIndex());
                if let Ok(folder) = dialog.GetFolder() {
                    println!("   folder: {}", path(&folder));
                }
                print_controls(&controls, &added);
            }
        }
        Ok(())
    }
}
