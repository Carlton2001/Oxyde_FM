use super::{selection, IdList};
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};
use windows::Win32::UI::Shell::{ILCreateFromPathW, ILFindLastID, SHOpenFolderAndSelectItems, SHParseDisplayName};

fn id_list(path: &str) -> IdList {
    let mut item = std::ptr::null_mut();
    unsafe { SHParseDisplayName(PCWSTR(HSTRING::from(path).as_ptr()), None, &mut item, 0, None) }.unwrap();
    IdList(item)
}

/// How a program names the folder it shows a file in.
#[derive(Clone, Copy)]
enum Named {
    /// `SHParseDisplayName`: through the known folder the folder is in.
    ThroughKnownFolder,
    /// `ILCreateFromPath`: through the drive.
    ThroughDrive,
}

/// What a program does to show a file in its folder, once the folder has a window.
fn show_in_folder(folder: &str, file: &str, named: Named) {
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok().unwrap();
        {
            let path = HSTRING::from(folder);
            let folder = match named {
                Named::ThroughKnownFolder => id_list(folder),
                Named::ThroughDrive => IdList(ILCreateFromPathW(PCWSTR(path.as_ptr()))),
            };
            let file = id_list(file);
            let child = ILFindLastID(file.0).cast_const();
            SHOpenFolderAndSelectItems(folder.0, Some(&[child]), 0).unwrap();
        }
        CoUninitialize();
    }
}

/// Needs the desktop Shell, which lists the windows: run with `--ignored` in a session.
/// The folder is in the user's profile, where the two ways to name it differ.
#[test]
#[ignore]
fn the_file_a_program_shows_in_the_folder_is_the_selection() {
    for named in [Named::ThroughKnownFolder, Named::ThroughDrive] {
        let folder = tempfile::tempdir().unwrap();
        let folder = folder.path().canonicalize().unwrap();
        let folder = folder.to_str().unwrap().trim_start_matches(r"\\?\").to_owned();
        let file = format!(r"{folder}\report.txt");
        std::fs::write(&file, b"").unwrap();
        let asked = {
            let (folder, file) = (folder.clone(), file.clone());
            move || drop(std::thread::spawn(move || show_in_folder(&folder, &file, named)))
        };
        let started = std::time::Instant::now();
        assert_eq!(selection(&folder, asked), [file]);
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
    }
}
