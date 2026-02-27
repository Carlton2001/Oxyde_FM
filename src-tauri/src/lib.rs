pub mod commands;
pub mod models;
pub mod utils;
pub mod systems;

use commands::archive::ArchiveState;
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, Emitter};
use crate::models::SnapRect;

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, SetWindowLongPtrW, GWLP_WNDPROC, WM_NCHITTEST, HTMAXBUTTON,
};
#[cfg(target_os = "windows")]
use std::sync::OnceLock;

#[cfg(target_os = "windows")]
static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();
#[cfg(target_os = "windows")]
static ORIGINAL_WNDPROC: OnceLock<isize> = OnceLock::new();

#[derive(Default)]
pub struct WindowState {
    pub maximize_button_rect: Mutex<Option<SnapRect>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
#[cfg(target_os = "windows")]
unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    const WM_DEVICECHANGE: u32 = 0x0219;
    const DBT_DEVICEARRIVAL: usize = 0x8000;
    const DBT_DEVICEREMOVECOMPLETE: usize = 0x8004;

    if msg == WM_DEVICECHANGE {
        let wp = wparam.0;
        if wp == DBT_DEVICEARRIVAL || wp == DBT_DEVICEREMOVECOMPLETE {
            if let Some(app) = APP_HANDLE.get() {
                let _ = app.emit("drives-changed", ());
            }
        }
    }

    if msg == WM_NCHITTEST {
        if let Some(app) = APP_HANDLE.get() {
            if let Some(state) = app.try_state::<WindowState>() {
                if let Some(rect) = *state.maximize_button_rect.lock().unwrap() {
                    let x = (lparam.0 & 0xffff) as i16 as i32;
                    let y = ((lparam.0 >> 16) & 0xffff) as i16 as i32;
                    
                    use windows::Win32::Graphics::Gdi::ScreenToClient;
                    use windows::Win32::Foundation::POINT;
                    let mut pt = POINT { x, y };
                    if ScreenToClient(hwnd, &mut pt).as_bool() {
                        if let Some(window) = app.get_webview_window("main") {
                            let sf = window.scale_factor().unwrap_or(1.0);
                            
                            let lx = pt.x as f64 / sf;
                            let ly = pt.y as f64 / sf;
                            
                            if lx >= rect.x && lx <= rect.x + rect.width &&
                               ly >= rect.y && ly <= rect.y + rect.height {
                                return LRESULT(HTMAXBUTTON as isize);
                            }
                        }
                    }
                }
            }
        }
    }

    let prev = ORIGINAL_WNDPROC.get().copied().unwrap_or(0);
    if prev != 0 {
        let prev_fn: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT =
            std::mem::transmute(prev);
        prev_fn(hwnd, msg, wparam, lparam)
    } else {
        use windows::Win32::UI::WindowsAndMessaging::DefWindowProcW;
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();


    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_window_state::Builder::new().build())

        .manage(ArchiveState(AtomicBool::new(false)))
        .manage(systems::file_ops::FileOperationManager::new())
        .manage(models::SessionManager::default())
        .manage(models::ConfigManager::new())
        .manage(models::HistoryManager::default())
        .manage(commands::duplicates::DuplicateSearchState::new())
        .invoke_handler(tauri::generate_handler![
            commands::io::list_dir,
            commands::system::get_drives,
            commands::system::open_item,
            commands::ops::delete_items,
            commands::ops::copy_items,
            commands::ops::move_items,
            commands::ops::cancel_file_operation,
            commands::ops::pause_file_operation,
            commands::ops::resume_file_operation,
            commands::ops::toggle_turbo,
            commands::ops::get_op_status,
            commands::system::get_accent_color,
            commands::io::get_file_properties,
            commands::io::get_files_summary,
            commands::io::show_system_properties,
            commands::search::start_search,
            commands::search::cancel_search,
            commands::io::rename_item,
            commands::io::create_dir,
            commands::ops::check_conflicts,
            commands::ops::restore_items,
            commands::ops::list_trash,
            commands::ops::empty_trash,
            commands::ops::purge_items,
            commands::ops::purge_recycle_bin,
            commands::ops::move_from_trash,
            commands::ops::get_history,
            commands::ops::undo_last_action,
            commands::ops::redo_last_action,
            commands::clipboard::get_clipboard_files,
            commands::clipboard::set_clipboard_files,
            commands::clipboard::set_clipboard_from_trash,
            commands::io::calculate_folder_size,
            commands::system::set_webview_background,
            commands::system::show_native_context_menu,
            commands::system::get_native_context_menu_items,
            commands::system::execute_native_menu_item,
            commands::system::get_mounted_images,
            commands::system::mount_disk_image,
            commands::system::unmount_disk_image,
            commands::system::oxide_sync_snap_rect,
            commands::system::get_quick_access_items,
            commands::system::add_to_quick_access,
            commands::system::remove_from_quick_access,
            commands::system::clear_app_cache,
            commands::system::restart_app,
            commands::io::set_shortcut_info,

            commands::icons::get_file_icon,
            commands::icons::purge_icon_cache,
            commands::thumbnails::get_image_thumbnail,
            commands::thumbnails::get_office_thumbnail,
            commands::thumbnails::get_office_text_preview,
            commands::archive::list_archive_contents,
            commands::archive::extract_archive,
            commands::archive::compress_to_archive,
            commands::archive::add_to_archive,
            commands::archive::cancel_archive_operation,
            // Session Commands
            commands::session::get_session_state,
            commands::session::create_tab,
            commands::session::close_tab,
            commands::session::switch_tab,
            commands::session::active_tab_navigate,
            commands::session::duplicate_tab,
            commands::session::close_other_tabs,
            commands::session::reorder_tabs,
            commands::session::set_active_panel,
            commands::session::update_sort_config,
            // Config Commands
            commands::config::get_config,
            commands::config::set_config_value,
            commands::config::reset_config_to_default,
            commands::sidebar::get_sidebar_nodes,
            commands::sidebar::get_subtree_nodes,
            commands::duplicates::find_duplicates,
            commands::duplicates::cancel_find_duplicates,
        ])
        .setup(|app| {
            use tauri::Manager;
            let config_manager = app.state::<models::ConfigManager>();
            if let Err(e) = config_manager.load(app.handle()) {
                eprintln!("Failed to load config: {:?}", e);
            }

            let session_manager = app.state::<models::SessionManager>();
            if let Err(e) = session_manager.load(app.handle()) {
                eprintln!("Failed to load session: {:?}", e);
            }

            // Register WindowState
            let window_state = WindowState::default();
            app.manage(window_state);

            #[cfg(target_os = "windows")]
            {
                use tauri::Manager;
                APP_HANDLE.set(app.handle().clone()).ok();
                
                if let Some(window) = app.get_webview_window("main") {
                    let hwnd = window.hwnd().unwrap();

                    unsafe {
                        let prev_wndproc = GetWindowLongPtrW(hwnd, GWLP_WNDPROC);
                        ORIGINAL_WNDPROC.set(prev_wndproc).unwrap();
                        SetWindowLongPtrW(hwnd, GWLP_WNDPROC, wndproc as usize as isize);
                    }
                }
            }

            // Start Quick Access Watcher
            systems::quick_access_watcher::setup_quick_access_watcher(app.handle().clone());

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
