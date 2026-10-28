//! GStreamer and its app sink, loaded at first use in a worker.
use super::super::linux_library::library;
pub(super) use super::super::linux_library::{GError, Object};
use std::ffi::{c_char, c_int, c_uint, c_void};

/// `GValue`, enough for a string, object or boxed property.
#[repr(C)]
pub(super) struct Value {
    pub kind: usize,
    pub data: [u64; 2],
}

/// The public head of `GParamSpec`.
#[repr(C)]
pub(super) struct ParamSpec {
    pub instance: *mut c_void,
    pub name: *const c_char,
    pub flags: c_uint,
    pub value_type: usize,
}

/// `GstMapInfo`.
#[repr(C)]
pub(super) struct MapInfo {
    pub memory: *mut c_void,
    pub flags: c_uint,
    pub data: *mut u8,
    pub size: usize,
    pub maxsize: usize,
    pub user_data: [*mut c_void; 4],
    pub reserved: [*mut c_void; 4],
}

/// `GstMiniObject`, the head of buffers and messages.
#[repr(C)]
pub(super) struct MiniObject {
    pub kind: usize,
    pub refcount: c_int,
    pub lockstate: c_int,
    pub flags: c_uint,
    pub copy: *const c_void,
    pub dispose: *const c_void,
    pub free: *const c_void,
    pub priv_uint: c_uint,
    pub priv_pointer: *mut c_void,
}

/// The public heads of `GstBuffer` and `GstMessage`, stable across GStreamer 1.x:
/// C reads these fields through macros, without accessors.
#[repr(C)]
pub(super) struct Buffer {
    pub mini_object: MiniObject,
    pub pool: *mut c_void,
    pub pts: u64,
}

#[repr(C)]
pub(super) struct Message {
    pub mini_object: MiniObject,
    pub kind: c_uint,
}

// The app sink library brings GStreamer core and GLib as dependencies.
library! { Gst from "libgstapp-1.0.so.0" {
    g_free: fn(*mut c_void);
    g_error_free: fn(*mut GError);
    g_value_init: fn(*mut Value, usize) -> *mut Value;
    g_value_set_string: fn(*mut Value, *const c_char);
    g_value_set_object: fn(*mut Value, Object);
    g_value_set_boxed: fn(*mut Value, Object);
    g_value_set_int: fn(*mut Value, c_int);
    g_value_set_uint: fn(*mut Value, c_uint);
    g_object_class_find_property: fn(*mut c_void, *const c_char) -> *const ParamSpec;
    g_signal_connect_data: fn(Object, *const c_char, *const c_void, *mut c_void, *const c_void, c_uint) -> std::ffi::c_ulong;
    g_value_unset: fn(*mut Value);
    g_object_set_property: fn(Object, *const c_char, *const Value);
    gst_init_check: fn(*mut c_int, *mut c_void, *mut *mut GError) -> c_int;
    gst_filename_to_uri: fn(*const c_char, *mut *mut GError) -> *mut c_char;
    gst_parse_launch: fn(*const c_char, *mut *mut GError) -> Object;
    gst_parse_bin_from_description: fn(*const c_char, c_int, *mut *mut GError) -> Object;
    gst_element_get_type: fn() -> usize;
    gst_caps_get_type: fn() -> usize;
    gst_caps_from_string: fn(*const c_char) -> Object;
    gst_bin_get_by_name: fn(Object, *const c_char) -> Object;
    gst_object_unref: fn(Object);
    gst_element_set_state: fn(Object, c_int) -> c_int;
    gst_element_seek_simple: fn(Object, c_int, c_int, i64) -> c_int;
    gst_element_query_position: fn(Object, c_int, *mut i64) -> c_int;
    gst_element_query_duration: fn(Object, c_int, *mut i64) -> c_int;
    gst_element_get_bus: fn(Object) -> Object;
    gst_bus_pop: fn(Object) -> *mut Message;
    gst_message_parse_error: fn(*mut Message, *mut *mut GError, *mut *mut c_char);
    gst_message_streams_selected_get_size: fn(*mut Message) -> c_uint;
    gst_message_streams_selected_get_stream: fn(*mut Message, c_uint) -> Object;
    gst_stream_get_stream_type: fn(Object) -> c_uint;
    gst_mini_object_unref: fn(*mut c_void);
    gst_app_sink_try_pull_sample: fn(Object, u64) -> Object;
    gst_app_sink_try_pull_preroll: fn(Object, u64) -> Object;
    gst_sample_get_buffer: fn(Object) -> *mut Buffer;
    gst_sample_get_caps: fn(Object) -> Object;
    gst_caps_get_structure: fn(Object, c_uint) -> Object;
    gst_structure_get_int: fn(Object, *const c_char, *mut c_int) -> c_int;
    gst_structure_get_fraction: fn(Object, *const c_char, *mut c_int, *mut c_int) -> c_int;
    gst_element_get_static_pad: fn(Object, *const c_char) -> Object;
    gst_pad_get_current_caps: fn(Object) -> Object;
    gst_buffer_map: fn(*mut Buffer, *mut MapInfo, c_uint) -> c_int;
    gst_buffer_unmap: fn(*mut Buffer, *mut MapInfo);
}}

pub(super) fn gst() -> Option<&'static Gst> {
    static GST: std::sync::OnceLock<Option<Gst>> = std::sync::OnceLock::new();
    GST.get_or_init(|| {
        let gst = Gst::load().inspect_err(|error| log::warn!("GStreamer unavailable: {error}")).ok()?;
        let mut error = std::ptr::null_mut();
        // SAFETY: no command-line arguments; GStreamer initializes once per process.
        if unsafe { (gst.gst_init_check)(std::ptr::null_mut(), std::ptr::null_mut(), &mut error) } == 0 {
            if !error.is_null() {
                // SAFETY: an error GStreamer handed over.
                unsafe { (gst.g_error_free)(error) };
            }
            log::warn!("GStreamer could not initialize");
            return None;
        }
        Some(gst)
    }).as_ref()
}
