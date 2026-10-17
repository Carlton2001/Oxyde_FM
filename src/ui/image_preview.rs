//! Preview presentation and worker dispatch; decoding lives in image_preview_decode.
use crate::ui::i18n::Message;
use gpui::{RenderImage, SvgRenderer};
#[cfg(test)]
use image::{DynamicImage, Frame, ImageFormat, RgbaImage};
use oxyde_lib::utils::image_preview_decode::{Policy, POLICY};
#[path = "image_preview/svg.rs"]
mod svg;
use std::{fmt, io::Read, path::Path, sync::Arc};
#[cfg(test)]
use std::{io::Cursor, time::Duration};
#[derive(Clone, Debug)]
pub(crate) enum PreviewError {
    Limit(&'static str),
    Read(String),
    Decode(String),
    Memory(String),
    Worker(String),
}

impl PreviewError {
    pub(crate) fn message(&self) -> Message {
        match self {
            Self::Limit(_) => Message::PreviewImageLimit,
            Self::Worker(detail) => worker_message(detail),
            _ => Message::PreviewUnavailable,
        }
    }
}

pub(crate) fn worker_message(detail: &str) -> Message {
    if detail.contains("PreviewWorkerTimeout:") {
        Message::PreviewTimedOut
    } else {
        Message::PreviewUnavailable
    }
}

impl fmt::Display for PreviewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Limit(limit) => write!(formatter, "Image preview exceeds {limit}"),
            Self::Read(error) | Self::Decode(error) | Self::Memory(error) | Self::Worker(error) => {
                formatter.write_str(error)
            }
        }
    }
}

#[derive(Clone)]
pub(crate) struct PreviewImage {
    pub(crate) image: Arc<RenderImage>,
    pub(crate) notice: Option<Message>,
    /// Intrinsic size of a vector source, displayed from the sharper raster.
    pub(crate) natural_size: Option<(u32, u32)>,
}

fn read_bounded(input: impl Read, limit: usize) -> Result<Vec<u8>, PreviewError> {
    oxyde_lib::utils::image_preview_decode::read_bounded(input, limit).map_err(convert_error)
}
fn convert_error(error: oxyde_lib::utils::image_preview_decode::PreviewError) -> PreviewError {
    use oxyde_lib::utils::image_preview_decode::PreviewError as DecodeError;
    match error {
        DecodeError::Limit(value) => PreviewError::Limit(value),
        DecodeError::Read(value) => PreviewError::Read(value),
        DecodeError::Decode(value) => PreviewError::Decode(value),
        DecodeError::Memory(value) => PreviewError::Memory(value),
    }
}
fn decode(
    data: &[u8],
    renderer: &SvgRenderer,
    policy: Policy,
) -> Result<PreviewImage, PreviewError> {
    let decoded = oxyde_lib::utils::image_preview_decode::decode(data, policy, &|data, policy| {
        svg::svg(data, renderer, policy)
    })
    .map_err(convert_error)?;
    let image = Arc::new(RenderImage::new(decoded.frames));
    crate::ui::preview_budget::admit(&image).map_err(PreviewError::Memory)?;
    Ok(PreviewImage {
        image,
        notice: decoded.limited.then_some(Message::PreviewAnimationStatic),
        natural_size: decoded.natural_size,
    })
}

pub(crate) fn load_path(path: &Path, renderer: &SvgRenderer) -> Result<PreviewImage, PreviewError> {
    load_path_mode(path, renderer, false)
}

pub(crate) fn load_thumbnail(path: &Path, renderer: &SvgRenderer) -> Result<PreviewImage, PreviewError> {
    load_path_mode(path, renderer, true)
}

fn load_path_mode(path: &Path, renderer: &SvgRenderer, thumbnail: bool) -> Result<PreviewImage, PreviewError> {
    #[cfg(target_os = "windows")]
    {
        // Retain the cheap admission and missing-file classification in the parent.
        let length = std::fs::metadata(path)
            .map_err(|error| PreviewError::Read(error.to_string()))?
            .len();
        if length > POLICY.encoded_bytes as u64 {
            return Err(PreviewError::Limit("encoded byte limit (64 MiB)"));
        }
        let _ = renderer;
        let reply = oxyde_lib::platform::platform()
            .run_preview_worker(if thumbnail { "image-thumbnail" } else { "image" }, path)
            .and_then(|bytes| oxyde_lib::utils::preview_protocol::decode(&bytes))
            .map_err(|error| PreviewError::Worker(error.to_string()))?;
        match reply {
            oxyde_lib::utils::preview_protocol::Reply::Images { frames, limited, natural_size } => {
                let image = Arc::new(RenderImage::new(frames));
                crate::ui::preview_budget::admit(&image).map_err(PreviewError::Memory)?;
                Ok(PreviewImage {
                    image,
                    notice: limited.then_some(Message::PreviewAnimationStatic),
                    natural_size,
                })
            }
            oxyde_lib::utils::preview_protocol::Reply::Error { limit: true, .. } => {
                Err(PreviewError::Limit("worker decoder limits"))
            }
            oxyde_lib::utils::preview_protocol::Reply::Error { detail, .. } => {
                Err(PreviewError::Decode(detail))
            }
            _ => Err(PreviewError::Worker("Unexpected preview reply".into())),
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let policy = if thumbnail { Policy { output_side: 128, frame_count: 1, ..POLICY } } else { POLICY };
        load_path_local_with_policy(path, renderer, policy)
    }
}

pub(crate) fn load_path_local(
    path: &Path,
    renderer: &SvgRenderer,
) -> Result<PreviewImage, PreviewError> {
    load_path_local_with_policy(path, renderer, POLICY)
}

fn load_path_local_with_policy(path: &Path, renderer: &SvgRenderer, policy: Policy) -> Result<PreviewImage, PreviewError> {
    let file = std::fs::File::open(path).map_err(|error| PreviewError::Read(error.to_string()))?;
    if file
        .metadata()
        .map_err(|error| PreviewError::Read(error.to_string()))?
        .len()
        > POLICY.encoded_bytes as u64
    {
        return Err(PreviewError::Limit("encoded byte limit (64 MiB)"));
    }
    let data = read_bounded(file, POLICY.encoded_bytes)?;
    decode(&data, renderer, policy)
}

#[cfg(test)]
#[path = "image_preview/tests/decoder_tests.rs"]
mod tests;
