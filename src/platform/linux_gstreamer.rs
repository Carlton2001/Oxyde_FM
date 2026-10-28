//! A playbin3 with an app sink, shared by hover videos and the Viewer player. Only
//! workers use it: a decoder can end its own process, never Oxyde.
use crate::models::CommandError;
use image::RgbaImage;
use library::{gst, GError, Gst, MapInfo, Object, Value};
use std::ffi::{c_int, c_void, CStr, CString};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::time::Duration;

mod library;

// GstState.
/// Worker side: initializing GStreamer scans the plugins that changed and writes the
/// registry the sandbox names.
pub(super) fn refresh_registry() -> std::io::Result<()> {
    gst().map(|_| ()).ok_or_else(|| std::io::Error::other("GStreamer unavailable"))
}

pub(super) const READY: i32 = 2;
pub(super) const PAUSED: i32 = 3;
pub(super) const PLAYING: i32 = 4;
const NULL: i32 = 1;
const STATE_CHANGE_FAILURE: i32 = 0;
// GstMessageType: extended types share bits with the basic ones.
const MESSAGE_EOS: u32 = 1 << 0;
const MESSAGE_ERROR: u32 = 1 << 1;
const MESSAGE_ASYNC_DONE: u32 = 1 << 21;
const MESSAGE_STREAMS_SELECTED: u32 = (1 << 31) + 5;
// GstStreamType, GstFormat, GstSeekFlags (flush, key unit, nearest), GstMapFlags.
const STREAM_VIDEO: u32 = 1 << 2;
const TIME: i32 = 3;
const SEEK: i32 = 1 | 4 | 32 | 64;
const MAP_READ: u32 = 1;
const INT: usize = 6 << 2;
const UINT: usize = 7 << 2;
const STRING: usize = 16 << 2;
/// Software decoders and converters otherwise start a thread per core, each decoder
/// thread holding frames: 760 MiB for a 4K HEVC video on 32 cores, beyond the worker's
/// memory, where allocations then fail into corrupted pictures and errors. With four,
/// FFmpeg decodes that video at 85 frames a second in 280 MiB.
const THREADS: u32 = 4;

/// A GStreamer object, released when dropped.
struct Owned(&'static Gst, Object);

impl Drop for Owned {
    fn drop(&mut self) {
        // SAFETY: the pointer came with a reference this value owns.
        unsafe { (self.0.gst_object_unref)(self.1) }
    }
}

fn error_text(gst: &Gst, error: *mut GError, fallback: &str) -> String {
    if error.is_null() {
        return fallback.into();
    }
    // SAFETY: an error GStreamer handed over, freed once read.
    unsafe {
        let message = CStr::from_ptr((*error).message).to_string_lossy().into_owned();
        (gst.g_error_free)(error);
        message
    }
}

/// playbin3's `element-setup`: bounds the threads of each element it adds, decoders
/// included, while they are still stopped.
unsafe extern "C" fn limit_threads(_player: Object, element: Object, _data: *mut c_void) {
    let Some(gst) = gst() else { return };
    // SAFETY: a live element, whose instance starts with its class (G_OBJECT_GET_CLASS).
    unsafe {
        let class = *element.cast::<*mut c_void>();
        for name in [c"max-threads", c"n-threads"] {
            let property = (gst.g_object_class_find_property)(class, name.as_ptr());
            if property.is_null() || !matches!((*property).value_type, INT | UINT) {
                continue;
            }
            let mut value = Value { kind: 0, data: [0; 2] };
            (gst.g_value_init)(&mut value, (*property).value_type);
            if (*property).value_type == INT {
                (gst.g_value_set_int)(&mut value, THREADS as c_int);
            } else {
                (gst.g_value_set_uint)(&mut value, THREADS);
            }
            (gst.g_object_set_property)(element, name.as_ptr(), &value);
            (gst.g_value_unset)(&mut value);
        }
    }
}

/// A pipeline may come back with a warning about an optional element.
fn forget_warning(gst: &Gst, error: &mut *mut GError) {
    if !error.is_null() {
        // SAFETY: an error GStreamer handed over, released once.
        unsafe { (gst.g_error_free)(*error) };
        *error = std::ptr::null_mut();
    }
}

/// What the pipeline posted since the last look.
#[derive(Default)]
pub(super) struct Events {
    pub error: Option<String>,
    pub ended: bool,
    /// A state change, or a seek, has completed.
    pub settled: bool,
    /// Once playbin3 has chosen its streams, whether one of them is a video.
    pub video: Option<bool>,
}

/// A decoded frame, released when dropped.
pub(super) struct Sample(&'static Gst, Object);

impl Drop for Sample {
    fn drop(&mut self) {
        // SAFETY: the reference the pull returned.
        unsafe { (self.0.gst_mini_object_unref)(self.1) }
    }
}

impl Sample {
    /// A BGRA copy within `side` pixels, and its timestamp when it has one.
    pub(super) fn image(&self, side: u32) -> Result<(RgbaImage, Option<Duration>), CommandError> {
        let gst = self.0;
        let invalid = || CommandError::Other("Invalid video frame".into());
        // SAFETY: a live sample; the buffer is mapped for reading and unmapped below.
        unsafe {
            let structure = (gst.gst_caps_get_structure)((gst.gst_sample_get_caps)(self.1), 0);
            let buffer = (gst.gst_sample_get_buffer)(self.1);
            let (mut width, mut height) = (0, 0);
            if structure.is_null() || buffer.is_null()
                || (gst.gst_structure_get_int)(structure, c"width".as_ptr(), &mut width) == 0
                || (gst.gst_structure_get_int)(structure, c"height".as_ptr(), &mut height) == 0
                || !(1..=side as i32).contains(&width) || !(1..=side as i32).contains(&height) {
                return Err(invalid());
            }
            let length = width as usize * height as usize * 4;
            let pts = ((*buffer).pts != u64::MAX).then(|| Duration::from_nanos((*buffer).pts));
            let mut map: MapInfo = std::mem::zeroed();
            if (gst.gst_buffer_map)(buffer, &mut map, MAP_READ) == 0 {
                return Err(invalid());
            }
            // BGRA rows of four-byte pixels are never padded.
            let pixels = (map.size == length && !map.data.is_null())
                .then(|| std::slice::from_raw_parts(map.data, length).to_vec());
            (gst.gst_buffer_unmap)(buffer, &mut map);
            let image = RgbaImage::from_raw(width as u32, height as u32, pixels.ok_or_else(invalid)?).ok_or_else(invalid)?;
            Ok((image, pts))
        }
    }

    /// Identifies the frame: a paused sink hands the same one over again, and pools
    /// reuse buffers, so their address alone could stand for a later frame.
    pub(super) fn identity(&self) -> (usize, u64) {
        // SAFETY: a live sample.
        unsafe {
            let buffer = (self.0.gst_sample_get_buffer)(self.1);
            (buffer as usize, if buffer.is_null() { 0 } else { (*buffer).pts })
        }
    }
}

/// playbin3 playing a file into an app sink named `frames`.
pub(super) struct Player {
    gst: &'static Gst,
    pipeline: Owned,
    sink: Owned,
    bus: Owned,
    /// The output's capsfilter named `size`, when it has one.
    size: Option<Owned>,
    /// The output's first element named `source`, which receives decoded frames.
    source: Option<Owned>,
}

impl Drop for Player {
    fn drop(&mut self) {
        // SAFETY: a live pipeline; NULL releases its decoders and threads.
        unsafe { (self.gst.gst_element_set_state)(self.pipeline.1, NULL) };
    }
}

impl Player {
    /// `player` describes playbin3, `output` the video sink bin; `state` starts it.
    pub(super) fn open(path: &Path, player: &CStr, output: &CStr, state: i32) -> Result<Self, CommandError> {
        let gst = gst().ok_or_else(|| CommandError::Other("Media playback needs GStreamer".into()))?;
        let name = CString::new(path.as_os_str().as_bytes()).map_err(|_| CommandError::Other("Invalid media path".into()))?;
        let mut error = std::ptr::null_mut();
        // SAFETY: valid strings; every returned object is checked, then owned.
        unsafe {
            let pipeline = (gst.gst_parse_launch)(player.as_ptr(), &mut error);
            if pipeline.is_null() {
                return Err(CommandError::Other(error_text(gst, error, "Cannot build the media pipeline")));
            }
            let pipeline = Owned(gst, pipeline);
            forget_warning(gst, &mut error);
            let handler: unsafe extern "C" fn(Object, Object, *mut c_void) = limit_threads;
            (gst.g_signal_connect_data)(pipeline.1, c"element-setup".as_ptr(), handler as *const c_void,
                std::ptr::null_mut(), std::ptr::null(), 0);
            let output = (gst.gst_parse_bin_from_description)(output.as_ptr(), 1, &mut error);
            if output.is_null() {
                (gst.gst_element_set_state)(pipeline.1, NULL);
                return Err(CommandError::Other(error_text(gst, error, "Cannot build the video output")));
            }
            forget_warning(gst, &mut error);
            let sink = (gst.gst_bin_get_by_name)(output, c"frames".as_ptr());
            let size = (gst.gst_bin_get_by_name)(output, c"size".as_ptr());
            let source = (gst.gst_bin_get_by_name)(output, c"source".as_ptr());
            // The bin's floating reference goes to playbin3, which owns it from here.
            let mut value = Value { kind: 0, data: [0; 2] };
            (gst.g_value_init)(&mut value, (gst.gst_element_get_type)());
            (gst.g_value_set_object)(&mut value, output);
            (gst.g_object_set_property)(pipeline.1, c"video-sink".as_ptr(), &value);
            (gst.g_value_unset)(&mut value);
            let bus = (gst.gst_element_get_bus)(pipeline.1);
            if sink.is_null() || bus.is_null() {
                for object in [sink, bus, size, source].into_iter().filter(|object| !object.is_null()) {
                    (gst.gst_object_unref)(object);
                }
                return Err(CommandError::Other("Incomplete media pipeline".into()));
            }
            let player = Self {
                gst,
                sink: Owned(gst, sink),
                bus: Owned(gst, bus),
                size: (!size.is_null()).then(|| Owned(gst, size)),
                source: (!source.is_null()).then(|| Owned(gst, source)),
                pipeline,
            };
            let uri = (gst.gst_filename_to_uri)(name.as_ptr(), &mut error);
            if uri.is_null() {
                return Err(CommandError::Other(error_text(gst, error, "Invalid media path")));
            }
            let mut value = Value { kind: 0, data: [0; 2] };
            (gst.g_value_init)(&mut value, STRING);
            (gst.g_value_set_string)(&mut value, uri);
            (gst.g_object_set_property)(player.pipeline.1, c"uri".as_ptr(), &value);
            (gst.g_value_unset)(&mut value);
            (gst.g_free)(uri.cast());
            if !player.set_state(state) {
                return Err(CommandError::Other(player.events().error.unwrap_or_else(|| "Cannot play the media".into())));
            }
            Ok(player)
        }
    }

    pub(super) fn set_state(&self, state: i32) -> bool {
        // SAFETY: a live pipeline.
        unsafe { (self.gst.gst_element_set_state)(self.pipeline.1, state) != STATE_CHANGE_FAILURE }
    }

    /// Every pending message, sorted by its exact type: a filtered pop for the end of
    /// stream would also take stream lists, whose extended type shares its bit.
    pub(super) fn events(&self) -> Events {
        let gst = self.gst;
        let mut events = Events::default();
        // SAFETY: a live bus; each message and stream is released once read.
        unsafe {
            loop {
                let message = (gst.gst_bus_pop)(self.bus.1);
                if message.is_null() {
                    return events;
                }
                match (*message).kind {
                    MESSAGE_ERROR if events.error.is_none() => {
                        let (mut error, mut debug) = (std::ptr::null_mut(), std::ptr::null_mut());
                        (gst.gst_message_parse_error)(message, &mut error, &mut debug);
                        (gst.g_free)(debug.cast());
                        events.error = Some(error_text(gst, error, "Media decoding failed"));
                    }
                    MESSAGE_EOS => events.ended = true,
                    MESSAGE_ASYNC_DONE => events.settled = true,
                    MESSAGE_STREAMS_SELECTED => {
                        events.video = Some((0..(gst.gst_message_streams_selected_get_size)(message)).any(|index| {
                            let stream = (gst.gst_message_streams_selected_get_stream)(message, index);
                            let video = !stream.is_null() && (gst.gst_stream_get_stream_type)(stream) & STREAM_VIDEO != 0;
                            if !stream.is_null() {
                                (gst.gst_object_unref)(stream);
                            }
                            video
                        }));
                    }
                    _ => {}
                }
                (gst.gst_mini_object_unref)(message.cast());
            }
        }
    }

    /// The next frame within `timeout`; a paused sink gives its preroll frame.
    pub(super) fn pull(&self, timeout: Duration, paused: bool) -> Option<Sample> {
        let nanoseconds = timeout.as_nanos() as u64;
        // SAFETY: a live app sink; the returned reference goes to `Sample`.
        let sample = unsafe {
            if paused {
                (self.gst.gst_app_sink_try_pull_preroll)(self.sink.1, nanoseconds)
            } else {
                (self.gst.gst_app_sink_try_pull_sample)(self.sink.1, nanoseconds)
            }
        };
        (!sample.is_null()).then(|| Sample(self.gst, sample))
    }

    pub(super) fn position(&self) -> Option<Duration> {
        let mut nanoseconds = 0;
        // SAFETY: a live pipeline and a writable integer.
        let known = unsafe { (self.gst.gst_element_query_position)(self.pipeline.1, TIME, &mut nanoseconds) } != 0;
        (known && nanoseconds >= 0).then(|| Duration::from_nanos(nanoseconds as u64))
    }

    pub(super) fn duration(&self) -> Option<Duration> {
        let mut nanoseconds = 0;
        // SAFETY: a live pipeline and a writable integer.
        let known = unsafe { (self.gst.gst_element_query_duration)(self.pipeline.1, TIME, &mut nanoseconds) } != 0;
        (known && nanoseconds > 0).then(|| Duration::from_nanos(nanoseconds as u64))
    }

    /// To the nearest key frame, as a progress bar drag needs.
    pub(super) fn seek(&self, position: Duration) -> bool {
        let nanoseconds = i64::try_from(position.as_nanos()).unwrap_or(i64::MAX);
        // SAFETY: a live pipeline.
        unsafe { (self.gst.gst_element_seek_simple)(self.pipeline.1, TIME, SEEK, nanoseconds) != 0 }
    }

    /// The decoded picture's display size, square pixels: stable while the output is
    /// scaled, unlike the frames, whose rounding would move the Viewer's layout.
    pub(super) fn source_size(&self) -> Option<(u32, u32)> {
        let source = self.source.as_ref()?;
        let gst = self.gst;
        // SAFETY: a live element; the pad and caps references are released below.
        unsafe {
            let pad = (gst.gst_element_get_static_pad)(source.1, c"sink".as_ptr());
            if pad.is_null() {
                return None;
            }
            let caps = (gst.gst_pad_get_current_caps)(pad);
            (gst.gst_object_unref)(pad);
            if caps.is_null() {
                return None;
            }
            let structure = (gst.gst_caps_get_structure)(caps, 0);
            let (mut width, mut height, mut numerator, mut denominator) = (0, 0, 1, 1);
            let known = !structure.is_null()
                && (gst.gst_structure_get_int)(structure, c"width".as_ptr(), &mut width) != 0
                && (gst.gst_structure_get_int)(structure, c"height".as_ptr(), &mut height) != 0;
            if known && (gst.gst_structure_get_fraction)(structure, c"pixel-aspect-ratio".as_ptr(), &mut numerator, &mut denominator) == 0 {
                (numerator, denominator) = (1, 1);
            }
            (gst.gst_mini_object_unref)(caps);
            if !known || width <= 0 || height <= 0 || numerator <= 0 || denominator <= 0 {
                return None;
            }
            let width = (i64::from(width) * i64::from(numerator) / i64::from(denominator)).clamp(1, i64::from(u32::MAX));
            Some((width as u32, height as u32))
        }
    }

    /// Replaces the caps of the `size` capsfilter, which renegotiates the scaler.
    pub(super) fn set_output_caps(&self, caps: &CStr) {
        let Some(size) = &self.size else { return };
        let gst = self.gst;
        // SAFETY: valid caps text; the value holds its own reference, released below.
        unsafe {
            let caps = (gst.gst_caps_from_string)(caps.as_ptr());
            if caps.is_null() {
                return;
            }
            let mut value = Value { kind: 0, data: [0; 2] };
            (gst.g_value_init)(&mut value, (gst.gst_caps_get_type)());
            (gst.g_value_set_boxed)(&mut value, caps);
            (gst.g_object_set_property)(size.1, c"caps".as_ptr(), &value);
            (gst.g_value_unset)(&mut value);
            (gst.gst_mini_object_unref)(caps);
        }
    }
}
