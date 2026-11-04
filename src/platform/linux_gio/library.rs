//! GIO entry points, loaded at first use: without GLib, remote locations alone are missing.
use super::super::linux_library::library;
pub(super) use super::super::linux_library::{GError, Object};
use std::ffi::{c_char, c_int, c_uint, c_ulong, c_void};

/// A GLib doubly linked list cell.
#[repr(C)]
pub(super) struct GList {
    pub data: *mut c_void,
    pub next: *mut GList,
    pub prev: *mut GList,
}

pub(super) type ReadyCallback = unsafe extern "C" fn(source: Object, result: Object, data: *mut c_void);

library! { Gio from "libgio-2.0.so.0" {
    g_free: fn(*mut c_void);
    g_error_free: fn(*mut GError);
    g_io_error_quark: fn() -> u32;
    g_object_unref: fn(Object);
    g_signal_connect_data: fn(Object, *const c_char, *const c_void, *mut c_void, *const c_void, c_uint) -> c_ulong;
    g_signal_handler_disconnect: fn(Object, c_ulong);
    g_main_context_new: fn() -> Object;
    g_main_context_unref: fn(Object);
    g_main_context_push_thread_default: fn(Object);
    g_main_context_pop_thread_default: fn(Object);
    g_main_context_iteration: fn(Object, c_int) -> c_int;
    g_file_new_for_uri: fn(*const c_char) -> Object;
    g_file_new_for_path: fn(*const c_char) -> Object;
    g_file_get_path: fn(Object) -> *mut c_char;
    g_file_get_uri: fn(Object) -> *mut c_char;
    g_file_get_child: fn(Object, *const c_char) -> Object;
    g_file_find_enclosing_mount: fn(Object, Object, *mut *mut GError) -> Object;
    g_file_mount_enclosing_volume: fn(Object, c_uint, Object, Object, Option<ReadyCallback>, *mut c_void);
    g_file_mount_enclosing_volume_finish: fn(Object, Object, *mut *mut GError) -> c_int;
    g_file_enumerate_children: fn(Object, *const c_char, c_uint, Object, *mut *mut GError) -> Object;
    g_file_enumerator_next_file: fn(Object, Object, *mut *mut GError) -> Object;
    g_file_info_get_name: fn(Object) -> *const c_char;
    g_file_info_get_display_name: fn(Object) -> *const c_char;
    g_file_info_get_file_type: fn(Object) -> c_uint;
    g_file_info_get_attribute_string: fn(Object, *const c_char) -> *const c_char;
    g_mount_get_name: fn(Object) -> *mut c_char;
    g_mount_get_root: fn(Object) -> Object;
    g_mount_unmount_with_operation: fn(Object, c_uint, Object, Object, Option<ReadyCallback>, *mut c_void);
    g_mount_unmount_with_operation_finish: fn(Object, Object, *mut *mut GError) -> c_int;
    g_mount_operation_new: fn() -> Object;
    g_mount_operation_set_username: fn(Object, *const c_char);
    g_mount_operation_set_password: fn(Object, *const c_char);
    g_mount_operation_set_domain: fn(Object, *const c_char);
    g_mount_operation_set_anonymous: fn(Object, c_int);
    g_mount_operation_set_password_save: fn(Object, c_uint);
    g_mount_operation_set_choice: fn(Object, c_int);
    g_mount_operation_reply: fn(Object, c_uint);
    g_cancellable_new: fn() -> Object;
    g_cancellable_cancel: fn(Object);
    g_strfreev: fn(*mut *mut c_char);
    g_bookmark_file_new: fn() -> *mut c_void;
    g_bookmark_file_free: fn(*mut c_void);
    g_bookmark_file_load_from_file: fn(*mut c_void, *const c_char, *mut *mut GError) -> c_int;
    g_bookmark_file_to_file: fn(*mut c_void, *const c_char, *mut *mut GError) -> c_int;
    g_bookmark_file_get_uris: fn(*mut c_void, *mut usize) -> *mut *mut c_char;
    g_bookmark_file_get_title: fn(*mut c_void, *const c_char, *mut *mut GError) -> *mut c_char;
    g_bookmark_file_set_title: fn(*mut c_void, *const c_char, *const c_char);
    g_bookmark_file_add_application: fn(*mut c_void, *const c_char, *const c_char, *const c_char);
    g_bookmark_file_remove_item: fn(*mut c_void, *const c_char, *mut *mut GError) -> c_int;
    g_list_free: fn(*mut GList);
    g_app_info_get_all: fn() -> *mut GList;
    g_app_info_get_all_for_type: fn(*const c_char) -> *mut GList;
    g_app_info_get_recommended_for_type: fn(*const c_char) -> *mut GList;
    g_app_info_get_default_for_type: fn(*const c_char, c_int) -> Object;
    g_app_info_get_id: fn(Object) -> *const c_char;
    g_app_info_get_display_name: fn(Object) -> *const c_char;
    g_app_info_get_icon: fn(Object) -> Object;
    g_app_info_should_show: fn(Object) -> c_int;
    g_app_info_launch_uris: fn(Object, *mut GList, Object, *mut *mut GError) -> c_int;
    g_app_info_set_as_default_for_type: fn(Object, *const c_char, *mut *mut GError) -> c_int;
    g_app_info_set_as_last_used_for_type: fn(Object, *const c_char, *mut *mut GError) -> c_int;
    g_desktop_app_info_new: fn(*const c_char) -> Object;
    g_app_launch_context_new: fn() -> Object;
    g_app_launch_context_unsetenv: fn(Object, *const c_char);
    g_icon_to_string: fn(Object) -> *mut c_char;
}}

pub(super) fn gio() -> Option<&'static Gio> {
    static GIO: std::sync::OnceLock<Option<Gio>> = std::sync::OnceLock::new();
    GIO.get_or_init(|| Gio::load().inspect_err(|error| log::warn!("GIO unavailable: {error}")).ok()).as_ref()
}
