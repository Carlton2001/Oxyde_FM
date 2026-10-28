//! poppler-glib and the cairo it draws with, loaded at first use in the worker.
use super::super::linux_library::library;
pub(super) use super::super::linux_library::{GError, Object};
use std::ffi::{c_char, c_int};

library! { Poppler from "libpoppler-glib.so.8" {
    g_object_unref: fn(Object);
    g_error_free: fn(*mut GError);
    poppler_document_new_from_fd: fn(c_int, *const c_char, *mut *mut GError) -> Object;
    poppler_document_get_n_pages: fn(Object) -> c_int;
    poppler_document_get_page: fn(Object, c_int) -> Object;
    poppler_page_get_size: fn(Object, *mut f64, *mut f64);
    poppler_page_render: fn(Object, Object);
    cairo_image_surface_create: fn(c_int, c_int, c_int) -> Object;
    cairo_image_surface_get_data: fn(Object) -> *mut u8;
    cairo_image_surface_get_stride: fn(Object) -> c_int;
    cairo_surface_status: fn(Object) -> c_int;
    cairo_surface_flush: fn(Object);
    cairo_surface_destroy: fn(Object);
    cairo_create: fn(Object) -> Object;
    cairo_destroy: fn(Object);
    cairo_status: fn(Object) -> c_int;
    cairo_scale: fn(Object, f64, f64);
    cairo_set_source_rgb: fn(Object, f64, f64, f64);
    cairo_paint: fn(Object);
}}

pub(super) fn poppler() -> Option<&'static Poppler> {
    static POPPLER: std::sync::OnceLock<Option<Poppler>> = std::sync::OnceLock::new();
    POPPLER.get_or_init(|| Poppler::load().inspect_err(|error| log::warn!("poppler unavailable: {error}")).ok()).as_ref()
}
