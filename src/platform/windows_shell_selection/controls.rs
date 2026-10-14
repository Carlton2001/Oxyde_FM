//! Activate common controls v6 only on the native properties thread.
use std::{io::Write, marker::PhantomData, rc::Rc};
use windows::core::{Error, PCWSTR};
use windows::Win32::Foundation::{FreeLibrary, HANDLE, HMODULE};
use windows::Win32::System::ApplicationInstallationAndServicing::{
    ActivateActCtx, CreateActCtxW, DeactivateActCtx, ReleaseActCtx, ACTCTXW,
};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::Win32::UI::Controls::{
    ICC_LINK_CLASS, ICC_STANDARD_CLASSES, ICC_WIN95_CLASSES, INITCOMMONCONTROLSEX,
};

pub(super) struct Controls {
    context: HANDLE,
    cookie: usize,
    library: HMODULE,
    _manifest: tempfile::TempPath,
    _thread: PhantomData<Rc<()>>,
}

impl Controls {
    pub(super) fn new() -> Result<Self, String> {
        let mut manifest = tempfile::NamedTempFile::new().map_err(|e| e.to_string())?;
        manifest.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
<assemblyIdentity version="1.0.0.0" processorArchitecture="*" name="Oxyde.NativeProperties" type="win32"/>
<dependency><dependentAssembly>
<assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/>
</dependentAssembly></dependency></assembly>"#).map_err(|e| e.to_string())?;
        manifest.flush().map_err(|e| e.to_string())?;
        let manifest = manifest.into_temp_path();
        let path: Vec<_> = manifest
            .to_string_lossy()
            .encode_utf16()
            .chain(Some(0))
            .collect();
        unsafe {
            let context = CreateActCtxW(&ACTCTXW {
                cbSize: std::mem::size_of::<ACTCTXW>() as u32,
                lpSource: PCWSTR(path.as_ptr()),
                ..Default::default()
            })
            .map_err(|e| e.to_string())?;
            let mut cookie = 0;
            if let Err(error) = ActivateActCtx(Some(context), &mut cookie) {
                ReleaseActCtx(context);
                return Err(error.to_string());
            }
            let mut controls = Self {
                context,
                cookie,
                library: HMODULE::default(),
                _manifest: manifest,
                _thread: PhantomData,
            };
            controls.library =
                LoadLibraryW(windows::core::w!("comctl32.dll")).map_err(|e| e.to_string())?;
            let initialize =
                GetProcAddress(controls.library, windows::core::s!("InitCommonControlsEx"))
                    .ok_or_else(|| "Missing InitCommonControlsEx".to_string())?;
            let initialize: unsafe extern "system" fn(
                *const INITCOMMONCONTROLSEX,
            ) -> windows::core::BOOL = std::mem::transmute(initialize);
            if !initialize(&INITCOMMONCONTROLSEX {
                dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
                dwICC: ICC_WIN95_CLASSES | ICC_STANDARD_CLASSES | ICC_LINK_CLASS,
            })
            .as_bool()
            {
                return Err(Error::from_thread().to_string());
            }
            Ok(controls)
        }
    }
}

impl Drop for Controls {
    fn drop(&mut self) {
        unsafe {
            if !self.library.is_invalid() {
                let _ = FreeLibrary(self.library);
            }
            let _ = DeactivateActCtx(0, self.cookie);
            ReleaseActCtx(self.context);
        }
    }
}
