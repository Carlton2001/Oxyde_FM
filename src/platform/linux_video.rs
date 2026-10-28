//! Muted hover videos through GStreamer, as Totem and the video thumbnailer Nautilus
//! runs decode them. Decoding stays in a self-worker; `stream` is the parent side.
use crate::models::CommandError;
use image::RgbaImage;
use library::{gst, GError, Gst, MapInfo, Object, Value};
use std::ffi::{CStr, CString};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::time::Duration;

mod library;
mod stream;

pub(super) use stream::stream;

/// Longest side of hover frames, as on Windows.
const SIDE: u32 = 640;
/// Video only, scaled by playbin's own converter, then BGRA for the UI. The sink
/// does not wait for the clock: the UI paces frames by their timestamps. playbin3
/// selects the video stream alone, where playbin still queued the ignored audio:
/// 205 MiB instead of 770 MiB for a 4K HEVC clip.
const PLAYER: &CStr = c"playbin3 flags=video";
const SINK: &CStr = c"videoconvert ! videoscale ! \
    video/x-raw,format=BGRA,pixel-aspect-ratio=1/1,width=[1,640],height=[1,640] ! \
    appsink name=frames sync=false max-buffers=2";
// GstState, GstMessageType, GstStreamType, GstMapFlags, G_TYPE_STRING.
const NULL: i32 = 1;
const READY: i32 = 2;
const PLAYING: i32 = 4;
const STATE_CHANGE_FAILURE: i32 = 0;
const MESSAGE_EOS: u32 = 1 << 0;
const MESSAGE_ERROR: u32 = 1 << 1;
const MESSAGE_STREAMS_SELECTED: u32 = (1 << 31) + 5;
const STREAM_VIDEO: u32 = 1 << 2;
const MAP_READ: u32 = 1;
const STRING: usize = 16 << 2;
/// Pause between loops, as on Windows.
const LOOP_PAUSE: Duration = Duration::from_millis(200);
const PULL_TIMEOUT_NS: u64 = 100_000_000;

/// A GStreamer object, released when dropped.
struct Owned(&'static Gst, Object);

impl Drop for Owned {
    fn drop(&mut self) {
        // SAFETY: the pointer came with a reference this value owns.
        unsafe { (self.0.gst_object_unref)(self.1) }
    }
}

/// The pipeline, stopped before its last reference goes.
struct Pipeline(Owned);

impl Drop for Pipeline {
    fn drop(&mut self) {
        // SAFETY: a live pipeline; NULL releases its decoders and threads.
        unsafe { (self.0 .0.gst_element_set_state)(self.0 .1, NULL) };
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

/// A pipeline may come back with a warning about an optional element.
fn forget_warning(gst: &Gst, error: &mut *mut GError) {
    if !error.is_null() {
        // SAFETY: an error GStreamer handed over, released once.
        unsafe { (gst.g_error_free)(*error) };
        *error = std::ptr::null_mut();
    }
}

fn open(gst: &'static Gst, path: &Path) -> Result<(Pipeline, Owned, Owned), CommandError> {
    let name = CString::new(path.as_os_str().as_bytes()).map_err(|_| CommandError::Other("Invalid video path".into()))?;
    let mut error = std::ptr::null_mut();
    // SAFETY: valid strings; every returned object is checked, then owned.
    unsafe {
        let pipeline = (gst.gst_parse_launch)(PLAYER.as_ptr(), &mut error);
        if pipeline.is_null() {
            return Err(CommandError::Other(error_text(gst, error, "Cannot build the video pipeline")));
        }
        let pipeline = Pipeline(Owned(gst, pipeline));
        forget_warning(gst, &mut error);
        let output = (gst.gst_parse_bin_from_description)(SINK.as_ptr(), 1, &mut error);
        if output.is_null() {
            return Err(CommandError::Other(error_text(gst, error, "Cannot build the video output")));
        }
        forget_warning(gst, &mut error);
        // The bin's floating reference goes to playbin3, which owns it from here.
        let sink = (gst.gst_bin_get_by_name)(output, c"frames".as_ptr());
        let mut value = Value { kind: 0, data: [0; 2] };
        (gst.g_value_init)(&mut value, (gst.gst_element_get_type)());
        (gst.g_value_set_object)(&mut value, output);
        (gst.g_object_set_property)(pipeline.0 .1, c"video-sink".as_ptr(), &value);
        (gst.g_value_unset)(&mut value);
        let uri = (gst.gst_filename_to_uri)(name.as_ptr(), &mut error);
        if uri.is_null() {
            return Err(CommandError::Other(error_text(gst, error, "Invalid video path")));
        }
        let mut value = Value { kind: 0, data: [0; 2] };
        (gst.g_value_init)(&mut value, STRING);
        (gst.g_value_set_string)(&mut value, uri);
        (gst.g_object_set_property)(pipeline.0 .1, c"uri".as_ptr(), &value);
        (gst.g_value_unset)(&mut value);
        (gst.g_free)(uri.cast());
        let bus = (gst.gst_element_get_bus)(pipeline.0 .1);
        if sink.is_null() || bus.is_null() {
            return Err(CommandError::Other("Incomplete video pipeline".into()));
        }
        let (sink, bus) = (Owned(gst, sink), Owned(gst, bus));
        if (gst.gst_element_set_state)(pipeline.0 .1, PLAYING) == STATE_CHANGE_FAILURE {
            return Err(CommandError::Other(events(gst, &bus).error.unwrap_or_else(|| "Cannot play the video".into())));
        }
        Ok((pipeline, sink, bus))
    }
}

/// What the pipeline posted since the last look.
#[derive(Default)]
struct Events {
    error: Option<String>,
    ended: bool,
    /// Once playbin3 has chosen its streams, whether one of them is a video.
    video: Option<bool>,
}

/// Every pending message, sorted by its exact type: extended types share bits with
/// the basic ones, so a filtered pop for the end of stream also takes stream lists.
fn events(gst: &Gst, bus: &Owned) -> Events {
    let mut events = Events::default();
    // SAFETY: a live bus; each message and stream is released once read.
    unsafe {
        loop {
            let message = (gst.gst_bus_pop)(bus.1);
            if message.is_null() {
                return events;
            }
            match (*message).kind {
                MESSAGE_ERROR if events.error.is_none() => {
                    let (mut error, mut debug) = (std::ptr::null_mut(), std::ptr::null_mut());
                    (gst.gst_message_parse_error)(message, &mut error, &mut debug);
                    (gst.g_free)(debug.cast());
                    events.error = Some(error_text(gst, error, "Video decoding failed"));
                }
                MESSAGE_EOS => events.ended = true,
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

/// A BGRA copy of the sample and its timestamp, the last one when it has none.
fn frame(gst: &Gst, sample: Object, previous: Duration) -> Result<(RgbaImage, Duration), CommandError> {
    let invalid = || CommandError::Other("Invalid video frame".into());
    // SAFETY: a live sample; the buffer is mapped for reading and unmapped below.
    unsafe {
        let structure = (gst.gst_caps_get_structure)((gst.gst_sample_get_caps)(sample), 0);
        let buffer = (gst.gst_sample_get_buffer)(sample);
        let (mut width, mut height) = (0, 0);
        if structure.is_null() || buffer.is_null()
            || (gst.gst_structure_get_int)(structure, c"width".as_ptr(), &mut width) == 0
            || (gst.gst_structure_get_int)(structure, c"height".as_ptr(), &mut height) == 0
            || !(1..=SIDE as i32).contains(&width) || !(1..=SIDE as i32).contains(&height) {
            return Err(invalid());
        }
        let length = width as usize * height as usize * 4;
        let pts = match (*buffer).pts {
            u64::MAX => previous,
            nanoseconds => Duration::from_nanos(nanoseconds),
        };
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

/// Worker side: frames in a loop until `on_frame` declines one.
pub(super) fn decode(path: &Path, on_frame: &mut dyn FnMut(RgbaImage, Duration) -> bool) -> Result<(), CommandError> {
    let gst = gst().ok_or_else(|| CommandError::Other("Video previews need GStreamer".into()))?;
    let (pipeline, sink, bus) = open(gst, path)?;
    let mut pts = Duration::ZERO;
    let mut looped_frame = false;
    // The bus says when the stream ended: the sink also reports an end before playbin3
    // has added it, once it knows the streams. Frames still queued come first.
    let mut ended = false;
    loop {
        let events = events(gst, &bus);
        if let Some(error) = events.error {
            return Err(CommandError::Other(error));
        }
        if events.video == Some(false) {
            return Err(CommandError::Other("No video stream".into()));
        }
        ended |= events.ended;
        let timeout = if ended { 0 } else { PULL_TIMEOUT_NS };
        // SAFETY: a live app sink; the sample is released once copied.
        let sample = unsafe { (gst.gst_app_sink_try_pull_sample)(sink.1, timeout) };
        if !sample.is_null() {
            let copied = frame(gst, sample, pts);
            // SAFETY: the reference the pull returned.
            unsafe { (gst.gst_mini_object_unref)(sample) };
            let (image, at) = copied?;
            pts = at;
            looped_frame = true;
            if !on_frame(image, pts) {
                return Ok(());
            }
        } else if ended {
            if !looped_frame {
                return Err(CommandError::Other("Video stream ended without a frame".into()));
            }
            std::thread::sleep(LOOP_PAUSE);
            // Like the Windows reader opened again; READY keeps the file.
            // SAFETY: a live pipeline.
            let restarted = unsafe {
                (gst.gst_element_set_state)(pipeline.0 .1, READY) != STATE_CHANGE_FAILURE
                    && (gst.gst_element_set_state)(pipeline.0 .1, PLAYING) != STATE_CHANGE_FAILURE
            };
            if !restarted {
                return Err(CommandError::Other("Cannot loop the video".into()));
            }
            looped_frame = false;
            ended = false;
        } else {
            // A sink that playbin3 has not started yet answers at once.
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

#[cfg(test)]
#[path = "linux_video/tests/video.rs"]
mod tests;
