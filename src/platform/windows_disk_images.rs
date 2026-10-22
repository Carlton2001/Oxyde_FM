use super::MountedDiskImage;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::OnceLock;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{RegisterWindowMessageW, WM_NCDESTROY};

static ACTIVE_MOUNTS: AtomicU32 = AtomicU32::new(0);
static AUTOPLAY_UNTIL: AtomicU64 = AtomicU64::new(0);
const AUTOPLAY_SUBCLASS: usize = 0x4f585944;
const AUTOPLAY_GRACE_MS: u64 = 1500;

fn elapsed_ms() -> u64 {
    static START: OnceLock<std::time::Instant> = OnceLock::new();
    START
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_millis() as u64
}

struct MountAutoplayGuard;
impl MountAutoplayGuard {
    fn new() -> Self {
        ACTIVE_MOUNTS.fetch_add(1, Ordering::AcqRel);
        Self
    }
}
impl Drop for MountAutoplayGuard {
    fn drop(&mut self) {
        // Device-arrival/AutoPlay notifications can arrive just after mount returns.
        AUTOPLAY_UNTIL.fetch_max(elapsed_ms() + AUTOPLAY_GRACE_MS, Ordering::AcqRel);
        ACTIVE_MOUNTS.fetch_sub(1, Ordering::AcqRel);
    }
}

fn cancel_mount_autoplay() -> bool {
    ACTIVE_MOUNTS.load(Ordering::Acquire) != 0
        || elapsed_ms() < AUTOPLAY_UNTIL.load(Ordering::Acquire)
}

unsafe extern "system" fn autoplay_subclass(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    id: usize,
    query_message: usize,
) -> LRESULT {
    if message == query_message as u32 && cancel_mount_autoplay() {
        // Shell asks the foreground window; TRUE cancels this AutoPlay event.
        return LRESULT(1);
    }
    if message == WM_NCDESTROY {
        unsafe {
            let _ = RemoveWindowSubclass(hwnd, Some(autoplay_subclass), id);
        }
    }
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

pub(super) fn install_autoplay_handler(hwnd: HWND) -> Result<(), String> {
    let query_message = unsafe { RegisterWindowMessageW(windows::core::w!("QueryCancelAutoPlay")) };
    if query_message == 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    if !unsafe {
        SetWindowSubclass(
            hwnd,
            Some(autoplay_subclass),
            AUTOPLAY_SUBCLASS,
            query_message as usize,
        )
    }
    .as_bool()
    {
        return Err("Cannot subclass the Oxyde window for AutoPlay".into());
    }
    Ok(())
}

mod native;

pub fn mounted_images() -> Result<Vec<MountedDiskImage>, String> {
    native::mounted_images()
}

pub fn mount(path: &str) -> Result<Vec<String>, String> {
    let path = crate::utils::path_security::validate_path(path).map_err(|error| error.to_string())?;
    if !path.is_file() || !crate::utils::disk_image::is_disk_image(&path) {
        return Err("Not a supported disk image file".into());
    }
    let _autoplay = MountAutoplayGuard::new();
    native::mount(&path)
}

pub fn unmount(path: &str) -> Result<(), String> {
    let path = crate::utils::path_security::validate_path(path).map_err(|error| error.to_string())?;
    native::unmount(&path)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_autoplay_query_is_cancelled_only_during_mount_and_grace() {
        use windows::Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, DestroyWindow, SendMessageW, WINDOW_EX_STYLE, WINDOW_STYLE,
        };
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                windows::core::w!("STATIC"),
                windows::core::w!("Oxyde AutoPlay test"),
                WINDOW_STYLE::default(),
                0,
                0,
                1,
                1,
                None,
                None,
                None,
                None,
            )
        }
        .unwrap();
        struct WindowCleanup(HWND);
        impl Drop for WindowCleanup {
            fn drop(&mut self) {
                unsafe {
                    let _ = DestroyWindow(self.0);
                }
            }
        }
        let _window = WindowCleanup(hwnd);
        install_autoplay_handler(hwnd).unwrap();
        let query = unsafe { RegisterWindowMessageW(windows::core::w!("QueryCancelAutoPlay")) };
        let ask = || unsafe { SendMessageW(hwnd, query, None, None).0 };
        assert_eq!(ask(), 0);
        let guard = MountAutoplayGuard::new();
        assert_eq!(ask(), 1);
        drop(guard);
        assert_eq!(ACTIVE_MOUNTS.load(Ordering::Acquire), 0);
        assert_eq!(ask(), 1);
        std::thread::sleep(std::time::Duration::from_millis(AUTOPLAY_GRACE_MS + 100));
        assert_eq!(ask(), 0);
    }

    #[test]
    #[ignore = "requires Windows Storage access and mounts only a generated test ISO"]
    fn windows_iso_mount_roundtrip() {
        let temp = tempfile::tempdir().unwrap();
        for extension in ["ISO", "img"] {
            let image = temp.path().join(format!("Oxyde épreuve '$().{extension}"));
            crate::commands::archive::test_iso_image(&image);
            let path = image.to_string_lossy().into_owned();
            struct Cleanup(String);
            impl Drop for Cleanup {
                fn drop(&mut self) {
                    let _ = unmount(&self.0);
                }
            }
            let cleanup = Cleanup(path.clone());
            let roots = mount(&path).unwrap();
            assert!(!roots.is_empty(), "mounted ISO has no accessible drive");
            assert!(std::path::Path::new(&roots[0]).join("DOCS").is_dir());
            assert_eq!(
                std::fs::read(std::path::Path::new(&roots[0]).join("DOCS/README.TXT")).unwrap(),
                b"ISO content"
            );
            let images = mounted_images().unwrap();
            assert!(
                images
                    .iter()
                    .any(|image| image.image_path.eq_ignore_ascii_case(&path)
                        && !image.roots.is_empty()),
                "image count: {}; exact path matches: {}; root counts: {:?}",
                images.len(),
                images
                    .iter()
                    .filter(|image| image.image_path.eq_ignore_ascii_case(&path))
                    .count(),
                images
                    .iter()
                    .map(|image| image.roots.len())
                    .collect::<Vec<_>>()
            );
            let repeated = mount(&path).unwrap();
            assert_eq!(repeated, roots);
            unmount(&path).unwrap();
            assert!(!mounted_images()
                .unwrap()
                .iter()
                .any(|image| image.image_path.eq_ignore_ascii_case(&path)));
            drop(cleanup);
        }
    }
}
