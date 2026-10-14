//! Interactive validation: inspect only windows belonging to this test process.
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::UI::WindowsAndMessaging::*;

struct Observed {
    marker: String,
    windows: Vec<HWND>,
    properties: bool,
    titles: Vec<String>,
}

struct DialogContent {
    properties: bool,
    error: bool,
}

unsafe extern "system" fn inspect_child(window: HWND, parameter: LPARAM) -> BOOL {
    let content = &mut *(parameter.0 as *mut DialogContent);
    let mut class = [0u16; 128];
    let count = GetClassNameW(window, &mut class);
    if String::from_utf16_lossy(&class[..count as usize]) == "SysTabControl32" {
        content.properties = SendMessageW(
            window,
            windows::Win32::UI::Controls::TCM_GETITEMCOUNT,
            None,
            None,
        )
        .0 > 0;
    }
    let mut text = [0u16; 512];
    let count = GetWindowTextW(window, &mut text);
    let text = String::from_utf16_lossy(&text[..count as usize]).to_lowercase();
    content.error |= text.contains("propriétés pour cet élément ne sont pas disponibles")
        || text.contains("properties for this item are not available");
    BOOL(1)
}

unsafe extern "system" fn inspect_window(window: HWND, parameter: LPARAM) -> BOOL {
    let mut process = 0;
    GetWindowThreadProcessId(window, Some(&mut process));
    if !IsWindowVisible(window).as_bool() {
        return BOOL(1);
    }
    let observed = &mut *(parameter.0 as *mut Observed);
    let mut title = [0u16; 512];
    let count = GetWindowTextW(window, &mut title);
    let title = String::from_utf16_lossy(&title[..count as usize]);
    if process != windows::Win32::System::Threading::GetCurrentProcessId()
        && !title.contains(&observed.marker)
    {
        return BOOL(1);
    }
    let mut content = DialogContent {
        properties: false,
        error: false,
    };
    let _ = EnumChildWindows(
        Some(window),
        Some(inspect_child),
        LPARAM(&mut content as *mut DialogContent as isize),
    );
    // A sheet can become visible before its pages have been constructed.
    // Closing it at that point would abort initialization and invalidate the test.
    if content.properties || content.error {
        observed.titles.push(title);
        observed.properties |= content.properties;
        observed.windows.push(window);
    }
    BOOL(1)
}

#[test]
#[ignore = "Opens native property windows; requires an interactive Windows desktop"]
fn merged_properties_open_a_real_property_sheet() {
    verify_property_sheet(["first.txt", "second.txt"]);
}

#[test]
#[ignore = "Opens native property windows; requires an interactive Windows desktop"]
fn mixed_files_open_a_real_property_sheet() {
    verify_property_sheet(["first.txt", "second.mp4"]);
}

fn verify_property_sheet(names: [&str; 2]) {
    let root = tempfile::tempdir().unwrap();
    let marker = format!(
        "oxyde-native-{}",
        root.path().file_name().unwrap().to_string_lossy()
    );
    let paths: Vec<_> = names
        .into_iter()
        .map(|name| {
            let path = root.path().join(format!("{marker}-{name}"));
            std::fs::write(&path, b"native properties test").unwrap();
            path.to_string_lossy().into_owned()
        })
        .collect();
    let done = Arc::new(AtomicBool::new(false));
    let completed = done.clone();
    let failed = Arc::new(AtomicBool::new(false));
    let failure = failed.clone();
    let worker = std::thread::spawn(move || {
        let result = super::show_properties(&paths);
        failure.store(result.is_err(), Ordering::Release);
        completed.store(true, Ordering::Release);
        result
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    let mut properties = false;
    let mut titles = Vec::new();
    while std::time::Instant::now() < deadline {
        let mut observed = Observed {
            marker: marker.clone(),
            windows: Vec::new(),
            properties: false,
            titles: Vec::new(),
        };
        unsafe {
            EnumWindows(
                Some(inspect_window),
                LPARAM(&mut observed as *mut Observed as isize),
            )
            .unwrap();
            properties |= observed.properties;
            if !observed.windows.is_empty() {
                titles.extend(observed.titles);
                for window in observed.windows {
                    let _ = PostMessageW(
                        Some(window),
                        WM_CLOSE,
                        windows::Win32::Foundation::WPARAM(0),
                        LPARAM(0),
                    );
                }
            }
        }
        if done.load(Ordering::Acquire) && (!titles.is_empty() || failed.load(Ordering::Acquire)) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert!(
        done.load(Ordering::Acquire),
        "Native properties did not return; observed {titles:?}"
    );
    let result = worker.join().unwrap();
    assert!(
        properties,
        "No property tabs appeared; observed {titles:?}; result {result:?}"
    );
    result.unwrap();
}
