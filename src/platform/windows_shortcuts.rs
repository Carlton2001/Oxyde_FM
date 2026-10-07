struct ComGuard;
impl Drop for ComGuard {
    fn drop(&mut self) {
        unsafe {
            windows::Win32::System::Com::CoUninitialize();
        }
    }
}

#[cfg(target_os = "windows")]
pub(super) fn get_shortcut_info(path: &std::path::Path) -> Option<crate::models::ShortcutInfo> {
    use windows::core::{Interface, HSTRING, PCWSTR};
    use windows::Win32::Storage::FileSystem::WIN32_FIND_DATAW;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, IPersistFile, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED, STGM_READ,
    };
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

    if !path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("lnk"))
    {
        return None;
    }

    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok().ok()?;
        let _com = ComGuard;

        let link: IShellLinkW = match CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) {
            Ok(l) => l,
            Err(_) => {
                return None;
            }
        };

        let persist: IPersistFile = match link.cast() {
            Ok(p) => p,
            Err(_) => {
                return None;
            }
        };

        let wide_path = HSTRING::from(path.to_string_lossy().as_ref());
        if persist.Load(PCWSTR(wide_path.as_ptr()), STGM_READ).is_err() {
            return None;
        }

        let mut target_buf = [0u16; 32768];
        let mut find_data = WIN32_FIND_DATAW::default();
        link.GetPath(&mut target_buf, &mut find_data, 4).ok()?;
        let target = String::from_utf16_lossy(&target_buf)
            .trim_matches('\0')
            .to_string();

        let mut args_buf = [0u16; 32768];
        link.GetArguments(&mut args_buf).ok()?;
        let arguments = String::from_utf16_lossy(&args_buf)
            .trim_matches('\0')
            .to_string();

        let mut dir_buf = [0u16; 32768];
        link.GetWorkingDirectory(&mut dir_buf).ok()?;
        let working_dir = String::from_utf16_lossy(&dir_buf)
            .trim_matches('\0')
            .to_string();

        let mut desc_buf = [0u16; 32768];
        link.GetDescription(&mut desc_buf).ok()?;
        let description = String::from_utf16_lossy(&desc_buf)
            .trim_matches('\0')
            .to_string();

        let run_window = link.GetShowCmd().map(|cmd| cmd.0).unwrap_or(1);

        Some(crate::models::ShortcutInfo {
            target,
            arguments,
            working_dir,
            description,
            run_window,
        })
    }
}

pub(super) fn set_shortcut_info(
    path: &str,
    info: &crate::models::ShortcutInfo,
) -> Result<(), String> {
    if !std::path::Path::new(path)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("lnk"))
    {
        return Err("Only .lnk files can be edited".into());
    }
    if info.target.trim().is_empty()
        || [
            info.target.as_str(),
            info.arguments.as_str(),
            info.working_dir.as_str(),
            info.description.as_str(),
        ]
        .iter()
        .any(|value| value.contains('\0'))
    {
        return Err("Invalid shortcut fields".into());
    }
    (|| -> Result<(), crate::models::CommandError> {
        use crate::models::CommandError;
        use std::path::PathBuf;
        use windows::core::{Interface, HSTRING, PCWSTR};
        use windows::Win32::System::Com::{
            CoCreateInstance, CoInitializeEx, IPersistFile, CLSCTX_INPROC_SERVER,
            COINIT_APARTMENTTHREADED, STGM_READWRITE,
        };
        use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

        let path_buf = PathBuf::from(&path);

        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED)
                .ok()
                .map_err(|e| CommandError::SystemError(e.to_string()))?;
            let _com = ComGuard;

            let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
                .map_err(|e| {
                    CommandError::SystemError(format!("CoCreateInstance failed: {}", e))
                })?;

            let persist: IPersistFile = link.cast().map_err(|e| {
                CommandError::SystemError(format!("Cast to IPersistFile failed: {}", e))
            })?;

            let wide_path = HSTRING::from(path_buf.to_string_lossy().as_ref());
            persist
                .Load(PCWSTR(wide_path.as_ptr()), STGM_READWRITE)
                .map_err(|e| CommandError::SystemError(format!("Load failed: {}", e)))?;

            let wide_target = HSTRING::from(info.target.as_str());
            link.SetPath(PCWSTR(wide_target.as_ptr()))
                .map_err(|e| CommandError::SystemError(format!("SetPath failed: {}", e)))?;

            let wide_args = HSTRING::from(info.arguments.as_str());
            link.SetArguments(PCWSTR(wide_args.as_ptr()))
                .map_err(|e| CommandError::SystemError(format!("SetArguments failed: {}", e)))?;

            let wide_dir = HSTRING::from(info.working_dir.as_str());
            link.SetWorkingDirectory(PCWSTR(wide_dir.as_ptr()))
                .map_err(|e| {
                    CommandError::SystemError(format!("SetWorkingDirectory failed: {}", e))
                })?;

            let wide_desc = HSTRING::from(info.description.as_str());
            link.SetDescription(PCWSTR(wide_desc.as_ptr()))
                .map_err(|e| CommandError::SystemError(format!("SetDescription failed: {}", e)))?;

            link.SetShowCmd(windows::Win32::UI::WindowsAndMessaging::SHOW_WINDOW_CMD(
                info.run_window,
            ))
            .map_err(|e| CommandError::SystemError(format!("SetShowCmd failed: {}", e)))?;

            persist
                .Save(PCWSTR(wide_path.as_ptr()), true)
                .map_err(|e| CommandError::SystemError(format!("Save failed: {}", e)))?;
        }
        Ok(())
    })()
    .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::core::{Interface, HSTRING, PCWSTR};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, IPersistFile, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
    #[test]
    fn shortcut_edits_preserve_icon_and_hotkey() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("test.lnk");
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok().unwrap();
            let _com = ComGuard;
            let link: IShellLinkW =
                CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).unwrap();
            link.SetPath(PCWSTR(HSTRING::from("C:\\original.exe").as_ptr()))
                .unwrap();
            link.SetIconLocation(PCWSTR(HSTRING::from("C:\\icons.dll").as_ptr()), 4)
                .unwrap();
            link.SetHotkey(0x0241).unwrap();
            let persist: IPersistFile = link.cast().unwrap();
            persist
                .Save(
                    PCWSTR(HSTRING::from(path.to_string_lossy().as_ref()).as_ptr()),
                    true,
                )
                .unwrap();
        }
        let mut info = get_shortcut_info(&path).unwrap();
        info.target = "C:\\edited.exe".into();
        info.arguments = "--name \"a b\"".into();
        info.working_dir = directory.path().to_string_lossy().into_owned();
        info.description = "updated".into();
        info.run_window = 3;
        set_shortcut_info(path.to_str().unwrap(), &info).unwrap();
        let loaded = get_shortcut_info(&path).unwrap();
        assert_eq!(loaded.target, info.target);
        assert_eq!(loaded.arguments, info.arguments);
        assert_eq!(loaded.working_dir, info.working_dir);
        assert_eq!(loaded.description, info.description);
        assert_eq!(loaded.run_window, 3);
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok().unwrap();
            let _com = ComGuard;
            let link: IShellLinkW =
                CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).unwrap();
            let persist: IPersistFile = link.cast().unwrap();
            persist
                .Load(
                    PCWSTR(HSTRING::from(path.to_string_lossy().as_ref()).as_ptr()),
                    windows::Win32::System::Com::STGM_READ,
                )
                .unwrap();
            let mut icon_buf = [0u16; 260];
            let mut icon_index = 0;
            link.GetIconLocation(&mut icon_buf, &mut icon_index).unwrap();
            assert_eq!(
                String::from_utf16_lossy(&icon_buf).trim_matches('\0'),
                "C:\\icons.dll",
            );
            assert_eq!(icon_index, 4);
            assert_eq!(link.GetHotkey().unwrap(), 0x0241);
        }
        info.target.clear();
        assert!(set_shortcut_info(path.to_str().unwrap(), &info).is_err());
        assert_eq!(get_shortcut_info(&path).unwrap().target, loaded.target);
    }
}
