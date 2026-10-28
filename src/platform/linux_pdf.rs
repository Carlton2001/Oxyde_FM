//! PDF pages through poppler, the renderer of Papers and of the thumbnailer Nautilus
//! runs. Rendering stays in a self-worker; `session` is the parent side.
use crate::models::CommandError;
use crate::utils::preview_protocol::{self, Reply};
use image::RgbaImage;
use library::{poppler, GError, Object, Poppler};
use std::io::{self, Read, Write};
use std::os::fd::IntoRawFd;
use std::path::Path;

mod library;
mod session;

pub(super) use session::{thumbnail, Session};

/// Hover previews, grid thumbnails and Viewer pages, as on Windows.
const HOVER_SIDE: u32 = crate::utils::image_preview_decode::POLICY.output_side;
const GRID_SIDE: u32 = 128;
const VIEWER_SIDE: u32 = preview_protocol::PREVIEW.side;
/// Page sizes are listed once; a larger document still lists its first pages.
const MAX_LISTED_PAGES: u32 = 100_000;
/// Viewer request that closes the document, then answered by `DOCUMENT_CLOSED`.
const CLOSE_DOCUMENT: u32 = u32::MAX;
const DOCUMENT_CLOSED: &str = "pdf-document-closed";
// cairo_format_t and cairo_status_t.
const ARGB32: i32 = 0;
const SUCCESS: i32 = 0;

/// A poppler or cairo instance, released when dropped.
struct Owned(Object, unsafe extern "C" fn(Object));

impl Owned {
    fn new(object: Object, release: unsafe extern "C" fn(Object), what: &str) -> Result<Self, CommandError> {
        if object.is_null() {
            return Err(CommandError::Other(format!("Cannot create the PDF {what}")));
        }
        Ok(Self(object, release))
    }
}

impl Drop for Owned {
    fn drop(&mut self) {
        // SAFETY: the pointer came with a reference this value owns.
        unsafe { (self.1)(self.0) }
    }
}

struct Document {
    poppler: &'static Poppler,
    document: Owned,
    pages: u32,
}

impl Document {
    fn open(path: &Path) -> Result<Self, CommandError> {
        let poppler = poppler().ok_or_else(|| CommandError::Other("PDF rendering needs poppler-glib".into()))?;
        let descriptor = std::fs::File::open(path)?.into_raw_fd();
        let mut error: *mut GError = std::ptr::null_mut();
        // SAFETY: poppler owns the descriptor from here and closes it with the document.
        let document = unsafe { (poppler.poppler_document_new_from_fd)(descriptor, std::ptr::null(), &mut error) };
        if document.is_null() {
            return Err(CommandError::Other(take_error(poppler, error)));
        }
        let document = Owned(document, poppler.g_object_unref);
        // SAFETY: a live document.
        let pages = unsafe { (poppler.poppler_document_get_n_pages)(document.0) }.max(0) as u32;
        if pages == 0 {
            return Err(CommandError::Other("Empty PDF".into()));
        }
        Ok(Self { poppler, document, pages })
    }

    /// Size in points, rotation applied.
    fn page(&self, index: u32) -> Result<(Owned, f64, f64), CommandError> {
        if index >= self.pages {
            return Err(CommandError::Other("PDF page is out of range".into()));
        }
        // SAFETY: an index within the document's pages.
        let page = unsafe { (self.poppler.poppler_document_get_page)(self.document.0, index as i32) };
        let page = Owned::new(page, self.poppler.g_object_unref, "page")?;
        let (mut width, mut height) = (0.0, 0.0);
        // SAFETY: a live page and two writable doubles.
        unsafe { (self.poppler.poppler_page_get_size)(page.0, &mut width, &mut height) };
        Ok((page, width, height))
    }

    /// `width height` lines, the format the Windows worker sends too.
    fn sizes(&self) -> Result<String, CommandError> {
        let mut sizes = String::new();
        for index in 0..self.pages.min(MAX_LISTED_PAGES) {
            let (_, width, height) = self.page(index)?;
            sizes.push_str(&format!("{width} {height}\n"));
        }
        Ok(sizes)
    }

    /// Premultiplied BGRA on white, the layout the UI uploads, within `max_side`.
    fn render(&self, index: u32, max_side: u32) -> Result<RgbaImage, CommandError> {
        let poppler = self.poppler;
        let (page, width, height) = self.page(index)?;
        let (columns, rows) = fit_within(width, height, max_side)?;
        // SAFETY: dimensions bounded by `max_side`; each object is checked before use.
        unsafe {
            let surface = Owned::new((poppler.cairo_image_surface_create)(ARGB32, columns as i32, rows as i32),
                poppler.cairo_surface_destroy, "surface")?;
            if (poppler.cairo_surface_status)(surface.0) != SUCCESS {
                return Err(CommandError::Other("Cannot allocate the PDF page".into()));
            }
            let context = Owned::new((poppler.cairo_create)(surface.0), poppler.cairo_destroy, "drawing")?;
            (poppler.cairo_set_source_rgb)(context.0, 1.0, 1.0, 1.0);
            (poppler.cairo_paint)(context.0);
            (poppler.cairo_scale)(context.0, f64::from(columns) / width, f64::from(rows) / height);
            (poppler.poppler_page_render)(page.0, context.0);
            if (poppler.cairo_status)(context.0) != SUCCESS {
                return Err(CommandError::Other("Cannot render the PDF page".into()));
            }
            drop(context);
            (poppler.cairo_surface_flush)(surface.0);
            let data = (poppler.cairo_image_surface_get_data)(surface.0);
            let stride = (poppler.cairo_image_surface_get_stride)(surface.0) as usize;
            let row = columns as usize * 4;
            if data.is_null() || stride < row {
                return Err(CommandError::Other("Invalid PDF page pixels".into()));
            }
            let mut pixels = Vec::new();
            pixels.try_reserve_exact(row * rows as usize)
                .map_err(|error| CommandError::Other(format!("Cannot allocate PDF page pixels: {error}")))?;
            for line in 0..rows as usize {
                pixels.extend_from_slice(std::slice::from_raw_parts(data.add(line * stride), row));
            }
            RgbaImage::from_raw(columns, rows, pixels).ok_or_else(|| CommandError::Other("Invalid PDF page pixels".into()))
        }
    }
}

fn take_error(poppler: &Poppler, error: *mut GError) -> String {
    if error.is_null() {
        return "Cannot open the PDF".into();
    }
    // SAFETY: a GError poppler handed over, freed once read.
    unsafe {
        let message = std::ffi::CStr::from_ptr((*error).message).to_string_lossy().into_owned();
        (poppler.g_error_free)(error);
        message
    }
}

/// Pixel size of a page scaled so that its longest side is `max_side`.
fn fit_within(width: f64, height: f64, max_side: u32) -> Result<(u32, u32), CommandError> {
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return Err(CommandError::Other("Invalid PDF page size".into()));
    }
    let longest = width.max(height);
    let fit = |side: f64| (side / longest * f64::from(max_side)).round().clamp(1.0, f64::from(max_side)) as u32;
    Ok((fit(width), fit(height)))
}

/// Worker side of a one-request preview: the first page, or why not.
pub(super) fn decode(mode: &str, path: &Path) -> Reply {
    let side = match mode {
        "pdf" => HOVER_SIDE,
        "pdf-grid" => GRID_SIDE,
        _ => return Reply::Error { limit: false, detail: format!("Unknown preview mode {mode}") },
    };
    reply(Document::open(path).and_then(|document| document.render(0, side)))
}

fn reply(page: Result<RgbaImage, CommandError>) -> Reply {
    match page {
        Ok(image) => Reply::Images { frames: vec![image::Frame::new(image)], limited: false, natural_size: None },
        Err(error) => Reply::Error { limit: false, detail: error.to_string() },
    }
}

fn frame(reply: Reply, output: &mut impl Write) -> io::Result<()> {
    let mut bytes = Vec::new();
    preview_protocol::encode(reply, &mut bytes)?;
    output.write_all(&(bytes.len() as u32).to_le_bytes())?;
    output.write_all(&bytes)?;
    output.flush()
}

/// Worker side of a Viewer document: page sizes first, then one frame per request.
pub(super) fn run_session(path: &Path) -> io::Result<()> {
    let mut output = io::stdout().lock();
    let document = match Document::open(path) {
        Ok(document) => document,
        Err(error) => return frame(Reply::Error { limit: false, detail: error.to_string() }, &mut output),
    };
    match document.sizes() {
        Ok(sizes) => frame(Reply::Text(sizes), &mut output)?,
        Err(error) => return frame(Reply::Error { limit: false, detail: error.to_string() }, &mut output),
    }
    let mut input = io::stdin().lock();
    let mut request = [0; 4];
    // The parent closing its end, or dying, ends the session too.
    while input.read_exact(&mut request).is_ok() {
        let index = u32::from_le_bytes(request);
        if index == CLOSE_DOCUMENT {
            drop(document);
            return frame(Reply::Text(DOCUMENT_CLOSED.into()), &mut output);
        }
        frame(reply(document.render(index, VIEWER_SIDE)), &mut output)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "linux_pdf/tests/pdf.rs"]
mod tests;
